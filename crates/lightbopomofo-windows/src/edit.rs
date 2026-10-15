//! Changing the document. TSF only allows edits inside an edit session, so
//! every change is a closure run by [`run`], and [`apply`] does the work:
//! commit text, then show the bopomofo still being composed (underlined) and
//! report where it is on screen for the candidate window.

use std::cell::RefCell;
use std::mem::ManuallyDrop;

use windows::Win32::Foundation::{E_FAIL, RECT};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::TextServices::{
    ITfComposition, ITfCompositionSink, ITfContext, ITfContextComposition, ITfEditSession,
    ITfEditSession_Impl, ITfInsertAtSelection, ITfRange, GUID_PROP_ATTRIBUTE,
    INSERT_TEXT_AT_SELECTION_FLAGS, TF_AE_NONE, TF_ANCHOR_END, TF_CONTEXT_EDIT_CONTEXT_FLAGS,
    TF_DEFAULT_SELECTION, TF_ES_ASYNCDONTCARE, TF_ES_READWRITE, TF_ES_SYNC, TF_SELECTION,
    TF_SELECTIONSTYLE,
};
use windows_core::{implement, Interface, Result, BOOL};

use crate::globals::{guard, wide};

type Edit = Box<dyn FnOnce(u32) -> Result<()>>;

#[implement(ITfEditSession)]
struct EditSession {
    edit: RefCell<Option<Edit>>,
}

impl ITfEditSession_Impl for EditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        guard("DoEditSession", Err(E_FAIL.into()), || {
            match self.edit.borrow_mut().take() {
                Some(edit) => edit(ec),
                None => Ok(()),
            }
        })
    }
}

/// Run `edit` in a read-write edit session: synchronously when the
/// application allows it (so text lands before the key is passed on),
/// otherwise as soon as it can.
pub fn run(
    context: &ITfContext,
    client: u32,
    edit: impl FnOnce(u32) -> Result<()> + 'static,
) -> Result<()> {
    let session: ITfEditSession = EditSession {
        edit: RefCell::new(Some(Box::new(edit))),
    }
    .into();
    let sync = TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_SYNC.0 | TF_ES_READWRITE.0);
    if let Ok(hr) = unsafe { context.RequestEditSession(client, &session, sync) } {
        if hr.is_ok() {
            return Ok(());
        }
    }
    let later = TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_ASYNCDONTCARE.0 | TF_ES_READWRITE.0);
    unsafe { context.RequestEditSession(client, &session, later)? }.ok()
}

/// Put the caret at the end of `range`.
fn cursor_after(ec: u32, context: &ITfContext, range: &ITfRange) -> Result<()> {
    unsafe {
        let end = range.Clone()?;
        end.Collapse(ec, TF_ANCHOR_END)?;
        let mut selection = [TF_SELECTION {
            range: ManuallyDrop::new(Some(end)),
            style: TF_SELECTIONSTYLE {
                ase: TF_AE_NONE,
                fInterimChar: BOOL(0),
            },
        }];
        let result = context.SetSelection(ec, &selection);
        ManuallyDrop::drop(&mut selection[0].range);
        result
    }
}

/// The current selection (where typing goes).
fn selection(ec: u32, context: &ITfContext) -> Result<ITfRange> {
    unsafe {
        let mut selection = [TF_SELECTION::default()];
        let mut fetched = 0;
        context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut selection, &mut fetched)?;
        let range = ManuallyDrop::take(&mut selection[0].range);
        range.ok_or_else(|| E_FAIL.into())
    }
}

/// Underline (display attribute) on `range`, or clear it with `atom == 0`.
fn mark(ec: u32, context: &ITfContext, range: &ITfRange, atom: u32) -> Result<()> {
    unsafe {
        let property = context.GetProperty(&GUID_PROP_ATTRIBUTE)?;
        if atom == 0 {
            property.Clear(ec, range)
        } else {
            property.SetValue(ec, range, &VARIANT::from(atom as i32))
        }
    }
}

/// Where the caret-side end of `range` is on screen.
pub fn screen_rect(ec: u32, context: &ITfContext, range: &ITfRange) -> Option<RECT> {
    unsafe {
        let view = context.GetActiveView().ok()?;
        let mut rect = RECT::default();
        let mut clipped = BOOL(0);
        view.GetTextExt(ec, range, &mut rect, &mut clipped).ok()?;
        (rect.right > rect.left || rect.bottom > rect.top).then_some(rect)
    }
}

/// The composition this text service owns in a context, if any.
#[derive(Default)]
pub struct Composition(RefCell<Option<ITfComposition>>);

impl Composition {
    pub fn forget(&self) {
        self.0.borrow_mut().take();
    }

    /// Commit `commit`, then show `composing` underlined at the caret.
    /// Returns the composing text's screen rectangle when there is some.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &self,
        ec: u32,
        context: &ITfContext,
        sink: &ITfCompositionSink,
        atom: u32,
        commit: &str,
        composing: &str,
    ) -> Result<Option<RECT>> {
        if !commit.is_empty() {
            let text = wide(commit);
            let current = self.0.borrow_mut().take();
            unsafe {
                match current {
                    Some(composition) => {
                        let range = composition.GetRange()?;
                        range.SetText(ec, 0, &text)?;
                        let _ = mark(ec, context, &range, 0);
                        cursor_after(ec, context, &range)?;
                        composition.EndComposition(ec)?;
                    }
                    None => {
                        let insert: ITfInsertAtSelection = context.cast()?;
                        let range = insert.InsertTextAtSelection(
                            ec,
                            INSERT_TEXT_AT_SELECTION_FLAGS(0),
                            &text,
                        )?;
                        cursor_after(ec, context, &range)?;
                    }
                }
            }
        }

        if composing.is_empty() {
            if let Some(composition) = self.0.borrow_mut().take() {
                unsafe {
                    let range = composition.GetRange()?;
                    range.SetText(ec, 0, &[])?;
                    composition.EndComposition(ec)?;
                }
            }
            return Ok(None);
        }

        let existing = self.0.borrow().clone();
        let composition = match existing {
            Some(composition) => composition,
            None => unsafe {
                let start = selection(ec, context)?;
                let contexts: ITfContextComposition = context.cast()?;
                let composition = contexts.StartComposition(ec, &start, sink)?;
                *self.0.borrow_mut() = Some(composition.clone());
                composition
            },
        };
        unsafe {
            let range = composition.GetRange()?;
            range.SetText(ec, 0, &wide(composing))?;
            let _ = mark(ec, context, &range, atom);
            cursor_after(ec, context, &range)?;
            Ok(screen_rect(ec, context, &range))
        }
    }

    /// The application ended the composition (focus moved, a click…):
    /// leave `text` in its place instead of raw bopomofo.
    pub fn terminated(&self, ec: u32, composition: &ITfComposition, text: &str) -> Result<()> {
        self.forget();
        unsafe {
            let range = composition.GetRange()?;
            range.SetText(ec, 0, &wide(text))?;
            if let Ok(context) = range.GetContext() {
                let _ = mark(ec, &context, &range, 0);
            }
        }
        Ok(())
    }
}
