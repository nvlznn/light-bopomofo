// C ABI of the 輕注音 engine. Hand-written for the MVP; keep in sync with
// crates/lightbopomofo-ffi/src/lib.rs.
//
// All strings are UTF-8. Strings returned by the engine must be released
// with lb_string_free. An engine must only be used from one thread.

#ifndef LIGHTBOPOMOFO_H
#define LIGHTBOPOMOFO_H

#include <stdint.h>

typedef struct LbEngine LbEngine;

// Returns NULL if the lexicon cannot be read. history_path may be NULL.
LbEngine *lb_engine_new(const char *lexicon_path, const char *history_path);
void lb_engine_free(LbEngine *engine);

// JSON array: [{"text":"你好嗎","consumed":3}, ...]; NULL on bad arguments.
char *lb_query(LbEngine *engine, const char *symbols);

// Record that the user committed `text` for the start of `symbols`.
void lb_learn(LbEngine *engine, const char *symbols, const char *text);

// Re-read the history file after the settings app changed it.
void lb_reload_history(LbEngine *engine);

// History file on its own, for the settings app (no lexicon loaded).
typedef struct LbHistory LbHistory;
LbHistory *lb_history_open(const char *path);
void lb_history_free(LbHistory *history);
// Re-reads the file. JSON array, most recent first:
// [{"typed":"ㄋㄏㄇ","text":"你好嗎","count":3,"last_used":1760000000}, ...]
char *lb_history_json(LbHistory *history);
// 1 removed, 0 not found, -1 error.
int32_t lb_history_forget(LbHistory *history, const char *typed, const char *text);
// 0 ok, -1 error.
int32_t lb_history_clear(LbHistory *history);

// 大千 key (unshifted US-layout character) -> bopomofo code point, or 0.
uint32_t lb_dachen(uint32_t key);

void lb_string_free(char *s);

#endif
