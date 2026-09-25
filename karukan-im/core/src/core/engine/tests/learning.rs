//! Tests for the learning cache and the Tab-skips-learning behavior.
//!
//! Space/Down: include learning candidates (default conversion).
//! Tab: skip learning candidates (lets users escape stale learned entries).
//! Ctrl+Delete: delete the selected learning candidate from the history.

use super::*;
use crate::core::engine::display::LEARNING_DELETE_HINT;

#[test]
fn build_candidates_includes_learning_when_not_skipped() {
    let mut engine = engine_with_learned("あい", "藍");

    let texts: Vec<String> = engine
        .build_conversion_candidates("あい", "あい", "", 9, LearningLookup::Use)
        .into_iter()
        .map(|c| c.text)
        .collect();

    assert!(
        texts.contains(&"藍".to_string()),
        "Space path (skip_learning=false) should surface learned `藍`, got {:?}",
        texts,
    );
}

#[test]
fn build_candidates_omits_learning_when_skipped() {
    let mut engine = engine_with_learned("あい", "藍");

    let texts: Vec<String> = engine
        .build_conversion_candidates("あい", "あい", "", 9, LearningLookup::Skip)
        .into_iter()
        .map(|c| c.text)
        .collect();

    assert!(
        !texts.contains(&"藍".to_string()),
        "Tab path (skip_learning=true) must drop learned `藍`, got {:?}",
        texts,
    );
}

#[test]
fn tab_key_skips_learning_in_composing() {
    // End-to-end: type the reading, press Tab → learned candidate is gone.
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    assert_eq!(engine.input_buf.reading(), "あい");

    let result = engine.process_key(&press_key(Keysym::TAB));
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    let texts: Vec<String> = engine
        .state()
        .candidates()
        .unwrap()
        .candidates()
        .iter()
        .map(|c| c.text.clone())
        .collect();
    assert!(
        !texts.contains(&"藍".to_string()),
        "Tab must skip the learned `藍` candidate, got {:?}",
        texts,
    );
}

#[test]
fn ctrl_delete_removes_selected_learning_entry() {
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    // Learning candidates are force-pushed first, so the learned entry is
    // the initial selection.
    let selected = engine
        .state()
        .candidates()
        .unwrap()
        .selected()
        .unwrap()
        .clone();
    assert_eq!(selected.text, "藍");
    assert!(
        selected.is_deletable(),
        "learning candidate must be flagged"
    );

    let result = engine.process_key(&press_ctrl(Keysym::DELETE));
    assert!(result.consumed);
    // The entry is gone from the cache...
    assert!(engine.learning.as_ref().unwrap().lookup("あい").is_empty());
    // ...and the window stays up: the conversion is rebuilt in place,
    // staying in Conversion.
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
    assert!(
        result
            .actions
            .iter()
            .any(|a| matches!(a, EngineAction::ShowCandidates(_))),
        "deletion must refresh the candidate window, not close it"
    );
    assert!(
        !result
            .actions
            .iter()
            .any(|a| matches!(a, EngineAction::HideCandidates)),
        "deletion must not hide the candidate window"
    );

    // `藍` is no longer a *learning* candidate. It may still return from the
    // model or dictionary (deleting history doesn't blacklist a surface — the
    // whole point of rebuilding instead of dropping the row), but never again
    // flagged as user history.
    let candidates = engine.state().candidates().unwrap();
    assert!(
        !candidates
            .candidates()
            .iter()
            .any(|c| c.text == "藍" && c.is_deletable()),
        "`藍` must no longer be a learning candidate after deletion",
    );
    // The rebuilt list reopens at the top.
    assert_eq!(candidates.cursor(), 0);
}

#[test]
fn ctrl_delete_removes_prefix_twins_so_surface_does_not_resurface() {
    // The same surface learned under two prefix-related readings is shown as a
    // single deduped row; deleting it must clear both, or the twin under the
    // longer reading pops back on the next conversion of the same input.
    let mut engine = engine_with_learned("あい", "藍");
    engine.learning.as_mut().unwrap().record("あいさ", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));

    let selected = engine
        .state()
        .candidates()
        .unwrap()
        .selected()
        .unwrap()
        .clone();
    assert_eq!(selected.text, "藍");
    assert!(selected.is_deletable());

    engine.process_key(&press_ctrl(Keysym::DELETE));
    // Both the exact and the prefix entry are gone.
    assert!(engine.learning.as_ref().unwrap().lookup("あい").is_empty());
    assert!(
        engine
            .learning
            .as_ref()
            .unwrap()
            .lookup("あいさ")
            .is_empty(),
        "the prefix twin (あいさ→藍) must be cleared too, not just the exact entry",
    );
}

#[test]
fn ctrl_delete_keeps_surface_that_another_source_also_produces() {
    // #2 regression: the learned surface equals the hiragana reading, which
    // the fallback ALWAYS produces. That fallback copy is deduped away under
    // the learning entry; deleting the entry must bring it back (now
    // non-learning) rather than remove the only row — which is why deletion
    // rebuilds the conversion instead of dropping the candidate in place.
    let mut engine = engine_with_learned("あい", "あい");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));

    let selected = engine
        .state()
        .candidates()
        .unwrap()
        .selected()
        .unwrap()
        .clone();
    assert_eq!(selected.text, "あい");
    assert!(selected.is_deletable());

    engine.process_key(&press_ctrl(Keysym::DELETE));
    assert!(engine.learning.as_ref().unwrap().lookup("あい").is_empty());

    // `あい` survives as an ordinary fallback candidate.
    let candidates = engine.state().candidates().unwrap();
    let ai = candidates.candidates().iter().find(|c| c.text == "あい");
    assert!(
        ai.is_some(),
        "the fallback `あい` must survive the deletion, not vanish with the \
         learning entry",
    );
    assert!(
        !ai.unwrap().is_deletable(),
        "the surviving `あい` must no longer be flagged as learning",
    );
}

#[test]
fn init_learning_cache_applies_configured_surface_cap() {
    // Guards the config→cache seam: if init_learning_cache stops applying
    // max_surface_chars, the 6-char surface (over the configured 5, under
    // the default 50) gets recorded and this fails.
    let mut engine = InputMethodEngine::new();
    engine.init_learning_cache(
        true,
        LearningConfig {
            max_entries: 10_000,
            max_surface_chars: 5,
        },
    );
    let cache = engine.learning.as_mut().expect("learning enabled");

    let before = cache.entry_count();
    cache.record("__karukan_seam_test__", &"漢".repeat(6));
    assert_eq!(
        cache.entry_count(),
        before,
        "a surface over the configured cap must be skipped; the configured \
         value is not reaching the cache",
    );
}

#[test]
fn ctrl_backspace_deletes_learning_entry_like_ctrl_delete() {
    // Mac keyboards label the Backspace key "delete", so the natural macOS
    // chord is Ctrl+delete = Ctrl+Backspace; it must behave like Ctrl+Delete
    // (forward delete), not like the plain-Backspace cancel.
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));

    let result = engine.process_key(&press_ctrl(Keysym::BACKSPACE));
    assert!(result.consumed);
    assert!(engine.learning.as_ref().unwrap().lookup("あい").is_empty());
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
}

#[test]
fn ctrl_backspace_does_nothing_for_non_learning_candidate() {
    // When the selection isn't a learning candidate, Ctrl+Backspace (like
    // Ctrl+Delete) is consumed so it can't leak to the app mid-conversion,
    // but the conversion is left intact. Cancelling stays on plain
    // Backspace / Escape.
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    // Move the selection off the learning candidate.
    engine.process_key(&press_key(Keysym::SPACE));
    let before = engine.state().candidates().unwrap().clone();
    assert!(!before.selected().unwrap().is_deletable());

    let result = engine.process_key(&press_ctrl(Keysym::BACKSPACE));
    assert!(
        result.consumed,
        "the chord must be consumed, not leak to the app"
    );
    assert!(
        matches!(engine.state(), InputState::Conversion { .. }),
        "Ctrl+Backspace must not cancel when the selection isn't deletable"
    );
    // Nothing changed: same selection, same list, history intact.
    let after = engine.state().candidates().unwrap();
    assert_eq!(after.cursor(), before.cursor());
    assert_eq!(after.selected_text(), before.selected_text());
    assert!(!engine.learning.as_ref().unwrap().lookup("あい").is_empty());
}

#[test]
fn ctrl_backspace_in_composing_deletes_char_not_history() {
    // History deletion is a Conversion-state-only chord. During Composing,
    // Ctrl+Backspace edits text like plain Backspace, even while a learning
    // suggestion is on screen.
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    assert!(matches!(engine.state(), InputState::Composing { .. }));

    let result = engine.process_key(&press_ctrl(Keysym::BACKSPACE));
    assert!(result.consumed);
    assert_eq!(engine.input_buf.reading(), "あ");
    assert!(
        !engine.learning.as_ref().unwrap().lookup("あい").is_empty(),
        "the learning entry must survive — deletion only works in Conversion",
    );
}

#[test]
fn ctrl_alt_delete_leaves_history_alone() {
    // Ctrl+Alt+Delete is a desktop chord, not ours. The delete arm guards on
    // `!alt_key` like its siblings, so the key passes through untouched
    // instead of irreversibly purging a history entry.
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(
        engine
            .state()
            .candidates()
            .unwrap()
            .selected()
            .unwrap()
            .is_deletable()
    );

    let result = engine.process_key(&press_ctrl_alt(Keysym::DELETE));
    assert!(!result.consumed, "Ctrl+Alt+Delete must reach the desktop");
    assert!(
        !engine.learning.as_ref().unwrap().lookup("あい").is_empty(),
        "Ctrl+Alt+Delete must not delete the learning entry"
    );
}

#[test]
fn plain_backspace_still_cancels_conversion() {
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));

    let result = engine.process_key(&press_key(Keysym::BACKSPACE));
    assert!(result.consumed);
    // Backspace without Ctrl keeps its cancel-to-composing behavior and
    // deletes nothing from the history.
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert!(!engine.learning.as_ref().unwrap().lookup("あい").is_empty());
}

#[test]
fn ctrl_delete_ignores_non_learning_candidate() {
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));

    // Move the selection off the learning candidate onto a fallback one.
    engine.process_key(&press_key(Keysym::SPACE));
    let selected = engine
        .state()
        .candidates()
        .unwrap()
        .selected()
        .unwrap()
        .clone();
    assert!(!selected.is_deletable());

    let before_len = engine.state().candidates().unwrap().len();
    let result = engine.process_key(&press_ctrl(Keysym::DELETE));
    // The key is consumed (it must not leak to the app mid-conversion) but
    // nothing is deleted and the conversion continues.
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
    assert_eq!(engine.state().candidates().unwrap().len(), before_len);
    assert!(!engine.learning.as_ref().unwrap().lookup("あい").is_empty());
}

#[test]
fn ctrl_delete_removes_prefix_matched_entry_by_full_reading() {
    // A prefix-matched learning candidate carries its own (longer) reading;
    // deletion must remove the cache entry under that full reading.
    let mut engine = engine_with_learned("あいさつ", "挨拶");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));

    let selected = engine
        .state()
        .candidates()
        .unwrap()
        .selected()
        .unwrap()
        .clone();
    assert_eq!(selected.text, "挨拶");
    assert_eq!(selected.reading.as_deref(), Some("あいさつ"));
    assert!(selected.is_deletable());

    engine.process_key(&press_ctrl(Keysym::DELETE));
    assert!(
        engine
            .learning
            .as_ref()
            .unwrap()
            .lookup("あいさつ")
            .is_empty()
    );
}

#[test]
fn aux_shows_delete_hint_only_for_learning_candidate() {
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));

    // Learning candidate selected → aux carries the deletion hint.
    let result = engine.process_key(&press_key(Keysym::SPACE));
    let aux = last_aux_text(&result).expect("conversion must update aux text");
    assert!(
        aux.contains(LEARNING_DELETE_HINT),
        "aux should show the deletion hint for a learning candidate, got {:?}",
        aux,
    );

    // Moving to a non-learning candidate drops the hint.
    let result = engine.process_key(&press_key(Keysym::SPACE));
    let aux = last_aux_text(&result).expect("navigation must update aux text");
    assert!(
        !aux.contains(LEARNING_DELETE_HINT),
        "aux must not show the deletion hint for non-learning candidates, got {:?}",
        aux,
    );
}

#[test]
fn space_key_keeps_learning_in_composing() {
    // Counterpart to tab_key_skips_learning_in_composing: Space stays on the
    // learning-included path so the default UX is unchanged.
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));

    let result = engine.process_key(&press_key(Keysym::SPACE));
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    let texts: Vec<String> = engine
        .state()
        .candidates()
        .unwrap()
        .candidates()
        .iter()
        .map(|c| c.text.clone())
        .collect();
    assert!(
        texts.contains(&"藍".to_string()),
        "Space must surface learned `藍`, got {:?}",
        texts,
    );
}

// ---- Long one-off commits stay out of the predictions ----------------

/// A live-converted sentence, committed once with Enter.
const SENTENCE_READING: &str = "きょうはかいぎがあるのではやめにかえります";
const SENTENCE: &str = "今日は会議があるので早めに帰ります";

/// Type `romaji` key by key and return the last result.
fn type_romaji(engine: &mut InputMethodEngine, romaji: &str) -> EngineResult {
    let mut result = None;
    for ch in romaji.chars() {
        result = Some(engine.process_key(&press(ch)));
    }
    result.expect("at least one key")
}

/// Texts of the candidate list a result shows (empty if it shows none).
fn shown_candidates(result: &EngineResult) -> Vec<String> {
    result
        .actions
        .iter()
        .rev()
        .find_map(|a| match a {
            EngineAction::ShowCandidates(list) => {
                Some(list.candidates().iter().map(|c| c.text.clone()).collect())
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// Texts of the open conversion list.
fn conversion_texts(engine: &InputMethodEngine) -> Vec<String> {
    engine
        .state()
        .candidates()
        .expect("conversion candidates")
        .candidates()
        .iter()
        .map(|c| c.text.clone())
        .collect()
}

#[test]
fn sentence_committed_once_is_not_predicted_from_its_first_kana() {
    // Before the cap, the sentence headed both the composing suggestions
    // and Space's list for every later 「きょう」 — Space+Enter would have
    // committed it in place of the four kana actually typed.
    let mut engine = engine_with_learned(SENTENCE_READING, SENTENCE);
    engine.learning.as_mut().unwrap().record("きょうと", "京都");

    let suggested = shown_candidates(&type_romaji(&mut engine, "kyou"));
    assert!(
        suggested.contains(&"京都".to_string()),
        "a learned word still predicts, got {suggested:?}",
    );
    assert!(
        !suggested.contains(&SENTENCE.to_string()),
        "a sentence committed once must not be suggested from 「きょう」, got {suggested:?}",
    );

    engine.process_key(&press_key(Keysym::SPACE));
    let texts = conversion_texts(&engine);
    assert!(texts.contains(&"京都".to_string()), "got {texts:?}");
    assert!(!texts.contains(&SENTENCE.to_string()), "got {texts:?}");
}

#[test]
fn a_long_entry_is_predicted_once_the_typing_is_within_reach() {
    // However often it was committed, the sentence stays out of the list
    // until at most `predict_extra_chars` (4) kana remain to type.
    let mut engine = engine_with_learned(SENTENCE_READING, SENTENCE);
    for _ in 0..3 {
        engine
            .learning
            .as_mut()
            .unwrap()
            .record(SENTENCE_READING, SENTENCE);
    }
    let suggested = shown_candidates(&type_romaji(&mut engine, "kyou"));
    assert!(
        !suggested.contains(&SENTENCE.to_string()),
        "got {suggested:?}"
    );
    engine.process_key(&press_key(Keysym::ESCAPE));
    assert!(matches!(engine.state(), InputState::Empty));

    // 「きょうはかいぎがあるのではやめにか」: four kana short of the end.
    let suggested = shown_candidates(&type_romaji(&mut engine, "kyouhakaigigaarunodehayamenika"));
    assert!(
        suggested.contains(&SENTENCE.to_string()),
        "within reach of its end the sentence completes, got {suggested:?}"
    );
}

#[test]
fn long_entry_still_matches_exactly() {
    // Eleven kana, over the default cap — typed in full, the learned
    // surface leads the list as it always did.
    let mut engine = engine_with_learned("おせわになっております", "お世話になっております");
    type_romaji(&mut engine, "osewaninatteorimasu");
    assert_eq!(engine.input_buf.reading(), "おせわになっております");

    engine.process_key(&press_key(Keysym::SPACE));
    let texts = conversion_texts(&engine);
    assert_eq!(
        texts.first().map(String::as_str),
        Some("お世話になっております"),
        "got {texts:?}"
    );
}

#[test]
fn long_entry_stays_in_the_learning_view() {
    // The Ctrl+R/T learning view is the history browser: everything, so a
    // held-back sentence can still be found (and deleted) there.
    let mut engine = engine_with_learned("おせわになっております", "お世話になっております");
    let suggested = shown_candidates(&type_romaji(&mut engine, "ose"));
    assert!(
        !suggested.contains(&"お世話になっております".to_string()),
        "got {suggested:?}"
    );

    // Ctrl+T while composing opens the conversion narrowed to the first
    // stop of the cycle, the learning view.
    engine.process_key(&press_ctrl(Keysym::KEY_T));
    let texts = conversion_texts(&engine);
    assert!(
        texts.contains(&"お世話になっております".to_string()),
        "the learning view keeps long entries, got {texts:?}",
    );
}
