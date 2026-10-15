//! The text service: one per thread that uses it. Receives keys, drives the
//! shared engine through a [`Composer`], edits the document and shows the
//! candidate window.

use std::cell::{Cell, RefCell};

use lightbopomofo_core::{Composer, Key};
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, LPARAM, WPARAM};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, IEnumTfDisplayAttributeInfo, ITfCategoryMgr, ITfCompartmentEventSink,
    ITfCompartmentEventSink_Impl, ITfCompartmentMgr, ITfComposition, ITfCompositionSink,
    ITfCompositionSink_Impl, ITfContext, ITfDisplayAttributeInfo, ITfDisplayAttributeProvider,
    ITfDisplayAttributeProvider_Impl, ITfDocumentMgr, ITfKeyEventSink, ITfKeyEventSink_Impl,
    ITfKeystrokeMgr, ITfSource, ITfTextInputProcessor, ITfTextInputProcessorEx,
    ITfTextInputProcessorEx_Impl, ITfTextInputProcessor_Impl, ITfThreadMgr, ITfThreadMgrEventSink,
    ITfThreadMgrEventSink_Impl, GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION,
    GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, TF_CONVERSIONMODE_NATIVE,
};
use windows_core::{implement, IUnknownImpl, Interface, Ref, Result, BOOL, GUID};

use crate::candidates;
use crate::display::{Attributes, InputAttribute};
use crate::edit::{self, Composition};
use crate::globals::{self, guard, GUID_DISPLAY_ATTRIBUTE_INPUT};
use crate::keys::{self, vk, Modifiers, ShiftTap};
use crate::log;

#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfThreadMgrEventSink,
    ITfCompositionSink,
    ITfDisplayAttributeProvider,
    ITfCompartmentEventSink
)]
pub struct TextService {
    thread_mgr: RefCell<Option<ITfThreadMgr>>,
    client: Cell<u32>,
    thread_sink_cookie: Cell<Option<u32>>,
    mode_sink_cookie: Cell<Option<u32>>,
    attribute_atom: Cell<u32>,
    composer: RefCell<Composer>,
    composition: Composition,
    /// Chinese mode (false: everything goes to the application).
    chinese: Cell<bool>,
    shift: RefCell<ShiftTap>,
}

impl TextService {
    pub fn new() -> Self {
        globals::add_ref();
        TextService {
            thread_mgr: RefCell::new(None),
            client: Cell::new(0),
            thread_sink_cookie: Cell::new(None),
            mode_sink_cookie: Cell::new(None),
            attribute_atom: Cell::new(0),
            composer: RefCell::new(Composer::new()),
            composition: Composition::default(),
            chinese: Cell::new(true),
            shift: RefCell::new(ShiftTap::default()),
        }
    }
}

impl Drop for TextService {
    fn drop(&mut self) {
        globals::release();
    }
}

fn pressed(key: u16) -> bool {
    (unsafe { GetKeyState(key as i32) }) < 0
}

fn modifiers() -> Modifiers {
    Modifiers {
        shift: pressed(vk::SHIFT),
        ctrl: pressed(vk::CONTROL),
        alt: pressed(vk::MENU),
        win: pressed(vk::LWIN) || pressed(vk::RWIN),
    }
}

fn caps_lock_on() -> bool {
    (unsafe { GetKeyState(VK_CAPITAL.0 as i32) }) & 1 != 0
}

fn variant_u32(value: u32) -> VARIANT {
    VARIANT::from(value as i32)
}

impl TextService_Impl {
    /// Whether this key is ours. Must have no side effects: TSF asks first
    /// (OnTestKeyDown) and only then delivers the key (OnKeyDown).
    fn wants(&self, vk_code: u16) -> Option<Key> {
        if !self.chinese.get() {
            return None;
        }
        let composing = !self.composer.borrow().is_empty();
        let key = keys::translate(vk_code, modifiers(), composing)?;
        if caps_lock_on() {
            // Caps Lock types English: a composition left over is dropped
            // instead of staying stuck in the document.
            return composing.then_some(Key::Abandon);
        }
        (composing || keys::starts_composition(&key)).then_some(key)
    }

    /// Bring the document and the candidate window up to date with the
    /// composer, committing `commit` first.
    fn render(&self, context: &ITfContext, commit: String) -> Result<()> {
        let (composing, items, selected) = {
            let composer = self.composer.borrow();
            let items: Vec<String> = composer
                .candidates()
                .iter()
                .map(|c| c.text.clone())
                .collect();
            (composer.composing_text(), items, composer.selected())
        };
        let this = self.to_object();
        let context_for_edit = context.clone();
        edit::run(context, self.client.get(), move |ec| {
            let sink: ITfCompositionSink = this.to_interface();
            let rect = this.composition.apply(
                ec,
                &context_for_edit,
                &sink,
                this.attribute_atom.get(),
                &commit,
                &composing,
            )?;
            if composing.is_empty() {
                candidates::hide();
            } else {
                candidates::show(items, selected, rect);
            }
            Ok(())
        })
    }

    fn handle(&self, context: &ITfContext, key: Key) -> Result<bool> {
        let Some(mut engine) = globals::engine() else {
            return Ok(false);
        };
        let result = self.composer.borrow_mut().handle(&mut engine, key);
        drop(engine);
        log::debug(|| {
            format!(
                "key handled={} commit={} chars, composing={} symbols",
                result.handled,
                result.commit.chars().count(),
                self.composer.borrow().composing_text().chars().count()
            )
        });
        if let Err(e) = self.render(context, result.commit) {
            // The application refused the edit: give up this composition and
            // let it have the key, rather than swallowing keys silently.
            log::error(&format!("edit failed: {e}"));
            self.composer.borrow_mut().reset();
            self.composition.forget();
            candidates::hide();
            return Ok(false);
        }
        Ok(result.handled)
    }

    /// The context with the keyboard focus, for changes not tied to a key.
    fn focused_context(&self) -> Option<ITfContext> {
        let thread_mgr = self.thread_mgr.borrow().clone()?;
        unsafe { thread_mgr.GetFocus().ok()?.GetTop().ok() }
    }

    /// Switch Chinese/English, dropping what is being composed.
    fn set_chinese(&self, chinese: bool, context: Option<ITfContext>) {
        if self.chinese.get() == chinese {
            return;
        }
        if !chinese {
            self.abandon(context);
        }
        self.chinese.set(chinese);
        self.publish_mode();
    }

    /// Drop the composition and erase its bopomofo from the document.
    fn abandon(&self, context: Option<ITfContext>) {
        if self.composer.borrow().is_empty() {
            return;
        }
        self.composer.borrow_mut().reset();
        candidates::hide();
        if let Some(context) = context.or_else(|| self.focused_context()) {
            let _ = self.render(&context, String::new());
        }
    }

    /// Tell Windows our mode, for the 中/英 indicator on the taskbar.
    fn publish_mode(&self) {
        let Some(thread_mgr) = self.thread_mgr.borrow().clone() else {
            return;
        };
        let mode = if self.chinese.get() {
            TF_CONVERSIONMODE_NATIVE
        } else {
            0
        };
        unsafe {
            if let Ok(compartments) = thread_mgr.cast::<ITfCompartmentMgr>() {
                if let Ok(c) = compartments.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) {
                    let opened = c.SetValue(self.client.get(), &variant_u32(1));
                    log::debug(|| format!("keyboard open: {opened:?}"));
                }
                if let Ok(c) =
                    compartments.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)
                {
                    let _ = c.SetValue(self.client.get(), &variant_u32(mode));
                }
            }
        }
    }

    fn activate(&self, thread_mgr: &ITfThreadMgr, client: u32) -> Result<()> {
        *self.thread_mgr.borrow_mut() = Some(thread_mgr.clone());
        self.client.set(client);
        unsafe {
            let keystrokes: ITfKeystrokeMgr = thread_mgr.cast()?;
            let key_sink: ITfKeyEventSink = self.to_interface();
            keystrokes.AdviseKeyEventSink(client, &key_sink, true)?;

            let source: ITfSource = thread_mgr.cast()?;
            let thread_sink: ITfThreadMgrEventSink = self.to_interface();
            self.thread_sink_cookie.set(
                source
                    .AdviseSink(&ITfThreadMgrEventSink::IID, &thread_sink)
                    .ok(),
            );

            if let Ok(categories) = CoCreateInstance::<_, ITfCategoryMgr>(
                &CLSID_TF_CategoryMgr,
                None,
                CLSCTX_INPROC_SERVER,
            ) {
                if let Ok(atom) = categories.RegisterGUID(&GUID_DISPLAY_ATTRIBUTE_INPUT) {
                    self.attribute_atom.set(atom);
                }
            }

            // Follow the 中/英 indicator when the user clicks it.
            let compartments: ITfCompartmentMgr = thread_mgr.cast()?;
            if let Ok(mode) =
                compartments.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)
            {
                let sink: ITfCompartmentEventSink = self.to_interface();
                if let Ok(source) = mode.cast::<ITfSource>() {
                    self.mode_sink_cookie
                        .set(source.AdviseSink(&ITfCompartmentEventSink::IID, &sink).ok());
                }
            }
        }
        self.chinese.set(true);
        self.publish_mode();
        // Load the lexicon now rather than on the first key.
        let loaded = globals::engine().is_some();
        log::info(&format!(
            "activated (engine {})",
            if loaded { "ready" } else { "unavailable" }
        ));
        Ok(())
    }

    fn deactivate(&self) -> Result<()> {
        // Switched to another input method: unchosen bopomofo goes away.
        self.abandon(None);
        candidates::destroy();
        self.composer.borrow_mut().reset();
        self.composition.forget();
        if let Some(thread_mgr) = self.thread_mgr.borrow_mut().take() {
            unsafe {
                if let Ok(keystrokes) = thread_mgr.cast::<ITfKeystrokeMgr>() {
                    let _ = keystrokes.UnadviseKeyEventSink(self.client.get());
                }
                if let (Ok(source), Some(cookie)) = (
                    thread_mgr.cast::<ITfSource>(),
                    self.thread_sink_cookie.take(),
                ) {
                    let _ = source.UnadviseSink(cookie);
                }
                if let (Ok(compartments), Some(cookie)) = (
                    thread_mgr.cast::<ITfCompartmentMgr>(),
                    self.mode_sink_cookie.take(),
                ) {
                    if let Ok(mode) =
                        compartments.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)
                    {
                        if let Ok(source) = mode.cast::<ITfSource>() {
                            let _ = source.UnadviseSink(cookie);
                        }
                    }
                }
            }
        }
        self.client.set(0);
        log::info("deactivated");
        Ok(())
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        ITfTextInputProcessorEx_Impl::ActivateEx(self, ptim, tid, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        guard("Deactivate", Ok(()), || self.deactivate())
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    fn ActivateEx(&self, ptim: Ref<ITfThreadMgr>, tid: u32, flags: u32) -> Result<()> {
        log::debug(|| format!("ActivateEx flags {flags:#x}"));
        guard("ActivateEx", Err(E_FAIL.into()), || {
            let thread_mgr = ptim.ok()?;
            self.activate(thread_mgr, tid).inspect_err(|e| {
                log::error(&format!("activate failed: {e}"));
            })
        })
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, foreground: BOOL) -> Result<()> {
        log::debug(|| format!("key foreground: {}", foreground.as_bool()));
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _pic: Ref<ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        guard("OnTestKeyDown", Ok(BOOL(0)), || {
            let vk_code = wparam.0 as u16;
            self.shift.borrow_mut().key_down(vk_code);
            let wanted = self.wants(vk_code).is_some();
            log::debug(|| {
                format!(
                    "test key {vk_code:#04x}: wanted={wanted} chinese={} caps={} composing={}",
                    self.chinese.get(),
                    caps_lock_on(),
                    !self.composer.borrow().is_empty()
                )
            });
            Ok(wanted.into())
        })
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        guard("OnKeyDown", Ok(BOOL(0)), || {
            let vk_code = wparam.0 as u16;
            self.shift.borrow_mut().key_down(vk_code);
            let Some(key) = self.wants(vk_code) else {
                return Ok(BOOL(0));
            };
            let context = pic.ok()?;
            Ok(self.handle(context, key)?.into())
        })
    }

    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        guard("OnTestKeyUp", Ok(BOOL(0)), || {
            Ok(self.shift.borrow().would_tap(wparam.0 as u16).into())
        })
    }

    fn OnKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        guard("OnKeyUp", Ok(BOOL(0)), || {
            if self.shift.borrow_mut().key_up(wparam.0 as u16) {
                self.set_chinese(!self.chinese.get(), pic.ok().ok().cloned());
            }
            // Never eat the key-up: the application must see Shift released,
            // or it would keep treating letters as shifted.
            Ok(BOOL(0))
        })
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(BOOL(0))
    }
}

impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnUninitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnSetFocus(&self, focus: Ref<ITfDocumentMgr>, _previous: Ref<ITfDocumentMgr>) -> Result<()> {
        log::debug(|| format!("document focus: {}", focus.is_some()));
        guard("ThreadMgr::OnSetFocus", Ok(()), || {
            // The old document ends its composition (OnCompositionTerminated).
            candidates::hide();
            Ok(())
        })
    }

    fn OnPushContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }

    fn OnPopContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    fn OnCompositionTerminated(
        &self,
        ecwrite: u32,
        pcomposition: Ref<ITfComposition>,
    ) -> Result<()> {
        guard("OnCompositionTerminated", Ok(()), || {
            let text = match globals::engine() {
                Some(mut engine) => self.composer.borrow_mut().commit_all(&mut engine),
                None => {
                    self.composer.borrow_mut().reset();
                    String::new()
                }
            };
            candidates::hide();
            match pcomposition.ok() {
                Ok(composition) => self.composition.terminated(ecwrite, composition, &text),
                Err(_) => {
                    self.composition.forget();
                    Ok(())
                }
            }
        })
    }
}

impl ITfDisplayAttributeProvider_Impl for TextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(Attributes::new().into())
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        if !guid.is_null() && unsafe { *guid } == GUID_DISPLAY_ATTRIBUTE_INPUT {
            Ok(InputAttribute.into())
        } else {
            Err(E_INVALIDARG.into())
        }
    }
}

impl ITfCompartmentEventSink_Impl for TextService_Impl {
    fn OnChange(&self, rguid: *const GUID) -> Result<()> {
        guard("Compartment::OnChange", Ok(()), || {
            if rguid.is_null()
                || unsafe { *rguid } != GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION
            {
                return Ok(());
            }
            let Some(thread_mgr) = self.thread_mgr.borrow().clone() else {
                return Ok(());
            };
            let value = unsafe {
                thread_mgr
                    .cast::<ITfCompartmentMgr>()?
                    .GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)?
                    .GetValue()?
            };
            let mode = i32::try_from(&value).unwrap_or(TF_CONVERSIONMODE_NATIVE as i32) as u32;
            self.set_chinese(mode & TF_CONVERSIONMODE_NATIVE != 0, None);
            Ok(())
        })
    }
}

// Interfaces implemented above, named so the compiler checks they exist.
#[allow(dead_code)]
fn _interfaces(
    _: ITfTextInputProcessor,
    _: ITfTextInputProcessorEx,
    _: ITfDisplayAttributeProvider,
) {
}
