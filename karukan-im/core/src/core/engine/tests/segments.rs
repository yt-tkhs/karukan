//! Segments (文節): Shift+←/→ split the reading, ←/→ move between the
//! pieces, each converts after the text before it, Enter commits them all.

use super::*;
use crate::core::preedit::AttributeType;

/// 「けっさいずみ」 with the model stubbed by the conversion cache: as one
/// reading it converts to 「決裁済み」; split, 「けっさい」 gives 「決済」 and
/// 「ずみ」 after it gives 「済み」 — after nothing, 「住み」, so a segment
/// converted without its predecessor as context would show.
fn engine_kessaizumi() -> InputMethodEngine {
    let mut engine = InputMethodEngine::new();
    seed_model_cache(&mut engine, "ケッサイズミ", "", &["決裁済み"]);
    seed_model_cache(&mut engine, "ケッサイ", "", &["決済"]);
    seed_model_cache(&mut engine, "ズミ", "決済", &["済み"]);
    seed_model_cache(&mut engine, "ズミ", "", &["住み"]);
    for ch in "kessaizumi".chars() {
        engine.process_key(&press(ch));
    }
    assert_eq!(engine.input_buf.reading(), "けっさいずみ");
    engine
}

/// Space, then Shift+← twice: 「けっさい｜ずみ」 with 「けっさい」 focused.
fn split_engine() -> InputMethodEngine {
    let mut engine = engine_kessaizumi();
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_shift_key(Keysym::LEFT));
    engine.process_key(&press_shift_key(Keysym::LEFT));
    assert_eq!(segment_readings(&engine), ["けっさい", "ずみ"]);
    engine
}

fn segment_readings(engine: &InputMethodEngine) -> Vec<String> {
    engine
        .state()
        .segments()
        .expect("conversion")
        .iter()
        .map(|s| s.reading.clone())
        .collect()
}

/// Texts of the focused segment's list — what the window shows.
fn focused_texts(engine: &InputMethodEngine) -> Vec<String> {
    engine
        .candidates()
        .expect("conversion candidates")
        .candidates()
        .iter()
        .map(|c| c.text.clone())
        .collect()
}

/// The preedit's styled ranges as (start, end, style).
fn preedit_ranges(engine: &InputMethodEngine) -> Vec<(usize, usize, AttributeType)> {
    engine
        .preedit()
        .expect("preedit")
        .attributes()
        .iter()
        .map(|a| (a.start, a.end, a.attr_type))
        .collect()
}

fn committed(result: &EngineResult) -> Option<String> {
    result.actions.iter().find_map(|a| match a {
        EngineAction::Commit(text) => Some(text.clone()),
        _ => None,
    })
}

fn shown(result: &EngineResult) -> Option<Vec<String>> {
    result.actions.iter().rev().find_map(|a| match a {
        EngineAction::ShowCandidates(list) => {
            Some(list.candidates().iter().map(|c| c.text.clone()).collect())
        }
        _ => None,
    })
}

#[test]
fn shift_left_splits_the_tail_off_the_focused_segment() {
    let mut engine = engine_kessaizumi();
    engine.process_key(&press_key(Keysym::SPACE));
    assert_eq!(
        engine.candidates().unwrap().selected_text(),
        Some("決裁済み")
    );
    assert!(!engine.state().is_segmented());

    engine.process_key(&press_shift_key(Keysym::LEFT));
    assert_eq!(segment_readings(&engine), ["けっさいず", "み"]);
    let result = engine.process_key(&press_shift_key(Keysym::LEFT));
    assert_eq!(segment_readings(&engine), ["けっさい", "ずみ"]);
    assert_eq!(engine.state().focus(), Some(0));

    // The preedit joins the segments, the focused one highlighted with the
    // caret at its end; the window shows the focused segment's list.
    let preedit = engine.preedit().unwrap();
    assert_eq!(preedit.text(), "決済済み");
    assert_eq!(
        preedit_ranges(&engine),
        vec![
            (0, 2, AttributeType::Highlight),
            (2, 4, AttributeType::Underline),
        ]
    );
    assert_eq!(preedit.caret(), 2);
    assert_eq!(shown(&result).unwrap()[0], "決済");
    let aux = last_aux_text(&result).unwrap();
    assert!(aux.contains("けっさい 4/30"), "aux was: {aux}");
}

#[test]
fn shift_right_extends_the_focused_segment_and_merges_back() {
    let mut engine = split_engine();

    engine.process_key(&press_shift_key(Keysym::RIGHT));
    assert_eq!(segment_readings(&engine), ["けっさいず", "み"]);
    engine.process_key(&press_shift_key(Keysym::RIGHT));
    assert_eq!(segment_readings(&engine), ["けっさいずみ"]);
    assert!(!engine.state().is_segmented());
    assert_eq!(
        engine.candidates().unwrap().selected_text(),
        Some("決裁済み")
    );

    // The last segment has nothing to take: inert, and consumed.
    let result = engine.process_key(&press_shift_key(Keysym::RIGHT));
    assert!(result.consumed);
    assert_eq!(segment_readings(&engine), ["けっさいずみ"]);
}

#[test]
fn a_segment_never_shrinks_below_one_char() {
    let mut engine = engine_kessaizumi();
    engine.process_key(&press_key(Keysym::SPACE));
    for _ in 0..6 {
        engine.process_key(&press_shift_key(Keysym::LEFT));
    }
    assert_eq!(segment_readings(&engine), ["け", "っさいずみ"]);
}

#[test]
fn arrows_move_the_focus_once_split() {
    let mut engine = split_engine();

    let result = engine.process_key(&press_key(Keysym::RIGHT));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
    assert_eq!(engine.state().focus(), Some(1));
    // 「ずみ」 converted after 「決済」: 済み, not the context-free 住み.
    assert_eq!(shown(&result).unwrap()[0], "済み");
    assert_eq!(
        preedit_ranges(&engine),
        vec![
            (0, 2, AttributeType::Underline),
            (2, 4, AttributeType::Highlight),
        ]
    );
    assert_eq!(engine.preedit().unwrap().caret(), 4);
    let aux = last_aux_text(&result).unwrap();
    assert!(aux.contains("ずみ 2/30"), "aux was: {aux}");

    // Past the last segment: inert.
    engine.process_key(&press_key(Keysym::RIGHT));
    assert_eq!(engine.state().focus(), Some(1));
    engine.process_key(&press_key(Keysym::LEFT));
    assert_eq!(engine.state().focus(), Some(0));
    engine.process_key(&press_key(Keysym::LEFT));
    assert_eq!(engine.state().focus(), Some(0));
    engine.process_key(&press_key(Keysym::END));
    assert_eq!(engine.state().focus(), Some(1));
    engine.process_key(&press_key(Keysym::HOME));
    assert_eq!(engine.state().focus(), Some(0));
    engine.process_key(&press_ctrl(Keysym::KEY_F));
    assert_eq!(engine.state().focus(), Some(1));
    engine.process_key(&press_ctrl(Keysym::KEY_B));
    assert_eq!(engine.state().focus(), Some(0));
}

#[test]
fn enter_commits_every_segment_and_learns_each() {
    let mut engine = split_engine();
    engine.learning = Some(LearningCache::new(LearningConfig::default()));

    let result = engine.process_key(&press_key(Keysym::RETURN));
    assert_eq!(committed(&result), Some("決済済み".to_string()));
    assert!(matches!(engine.state(), InputState::Empty));

    let cache = engine.learning.as_ref().unwrap();
    assert_eq!(cache.lookup("けっさい")[0].0, "決済");
    assert_eq!(cache.lookup("ずみ")[0].0, "済み");
    assert!(cache.lookup("けっさいずみ").is_empty());
}

#[test]
fn escape_returns_to_the_whole_composition() {
    let mut engine = split_engine();
    engine.process_key(&press_key(Keysym::ESCAPE));
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.input_buf.reading(), "けっさいずみ");
    assert_eq!(engine.preedit().unwrap().text(), "けっさいずみ");
}

#[test]
fn typing_commits_every_segment_and_continues() {
    let mut engine = split_engine();
    let result = engine.process_key(&press('n'));
    assert_eq!(committed(&result), Some("決済済み".to_string()));
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.input_buf.display(), "n");
    assert_eq!(
        engine.surrounding_context.as_ref().unwrap().left.as_deref(),
        Some("決済済み")
    );
}

#[test]
fn ctrl_digit_picks_and_moves_on_to_the_next_segment() {
    let mut engine = split_engine();

    let result = engine.process_key(&press_ctrl(Keysym::KEY_1));
    assert!(
        committed(&result).is_none(),
        "the pick moves on, it does not commit"
    );
    assert_eq!(engine.state().focus(), Some(1));
    assert_eq!(
        engine.state().segments().unwrap()[0].selected_text(),
        "決済"
    );

    // On the last segment the pick commits the whole, as it always did.
    let result = engine.process_key(&press_ctrl(Keysym::KEY_1));
    assert_eq!(committed(&result), Some("決済済み".to_string()));
    assert!(matches!(engine.state(), InputState::Empty));
}

#[test]
fn shift_left_while_composing_splits_straight_away() {
    let mut engine = engine_kessaizumi();
    let result = engine.process_key(&press_shift_key(Keysym::LEFT));
    assert!(result.consumed);
    assert_eq!(segment_readings(&engine), ["けっさいず", "み"]);
    assert_eq!(engine.state().focus(), Some(0));

    // A one-char reading has nothing to split off: plain conversion.
    let mut engine = InputMethodEngine::new();
    engine.process_key(&press('a'));
    let result = engine.process_key(&press_shift_key(Keysym::LEFT));
    assert!(result.consumed);
    assert_eq!(segment_readings(&engine), ["あ"]);
    assert!(
        result
            .actions
            .iter()
            .any(|a| matches!(a, EngineAction::ShowCandidates(_)))
    );
}

#[test]
fn resizing_keeps_the_source_filter_and_each_segment_keeps_its_own() {
    let mut engine = engine_kessaizumi();
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_ctrl(Keysym::KEY_I));
    assert_eq!(engine.state().filter(), Some(CandidateSource::Model));

    engine.process_key(&press_shift_key(Keysym::LEFT));
    engine.process_key(&press_shift_key(Keysym::LEFT));
    assert_eq!(segment_readings(&engine), ["けっさい", "ずみ"]);
    assert_eq!(engine.state().filter(), Some(CandidateSource::Model));
    assert_eq!(focused_texts(&engine), vec!["決済"]);

    // The next segment opens on its full list; coming back finds the view.
    engine.process_key(&press_key(Keysym::RIGHT));
    assert_eq!(engine.state().filter(), None);
    assert_eq!(focused_texts(&engine)[0], "済み");
    assert!(
        focused_texts(&engine).len() > 1,
        "the full list, kana pair included"
    );
    engine.process_key(&press_key(Keysym::LEFT));
    assert_eq!(engine.state().filter(), Some(CandidateSource::Model));
}

#[test]
fn commit_api_commits_every_segment() {
    let mut engine = split_engine();
    assert_eq!(engine.commit(), "決済済み");
    assert!(matches!(engine.state(), InputState::Empty));
}

#[test]
fn shift_arrows_are_inert_in_the_emoji_picker() {
    let mut engine = InputMethodEngine::new();
    engine.process_key(&press(':'));
    for ch in "smile".chars() {
        engine.process_key(&press(ch));
    }
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    let result = engine.process_key(&press_shift_key(Keysym::LEFT));
    assert!(result.consumed);
    assert!(!engine.state().is_segmented());
    assert_eq!(engine.mode.current(), InputMode::Emoji);
}

#[test]
fn a_closed_segment_takes_no_predictive_matches() {
    // A candidate whose reading ran past the boundary (けっさいず → PRED)
    // would double up with the next segment on commit; the last segment
    // still predicts (ずみか → PRED2), it ends where typing ended.
    let mut engine = engine_kessaizumi();
    engine.dicts.user = Some(dict_from_json(
        r#"[
            {"reading":"けっさいず","candidates":[{"surface":"PRED","score":1.0}]},
            {"reading":"ずみか","candidates":[{"surface":"PRED2","score":1.0}]}
        ]"#,
    ));
    let mut cache = LearningCache::new(LearningConfig::default());
    cache.record("けっさいず", "LEARNED");
    cache.record("けっさいず", "LEARNED");
    engine.learning = Some(cache);

    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_shift_key(Keysym::LEFT));
    engine.process_key(&press_shift_key(Keysym::LEFT));
    assert_eq!(segment_readings(&engine), ["けっさい", "ずみ"]);

    let head = focused_texts(&engine);
    assert!(!head.iter().any(|t| t == "PRED"), "got {head:?}");
    assert!(!head.iter().any(|t| t == "LEARNED"), "got {head:?}");

    engine.process_key(&press_key(Keysym::RIGHT));
    let tail = focused_texts(&engine);
    assert!(tail.iter().any(|t| t == "PRED2"), "got {tail:?}");

    // The dictionary view of the closed segment is exact-only too (two
    // Ctrl+T steps along the cycle: learning, then the dictionaries).
    engine.process_key(&press_key(Keysym::LEFT));
    engine.process_key(&press_ctrl(Keysym::KEY_T));
    engine.process_key(&press_ctrl(Keysym::KEY_T));
    assert_eq!(engine.state().filter(), Some(CandidateSource::Dictionary));
    let view = focused_texts(&engine);
    assert!(!view.iter().any(|t| t == "PRED"), "got {view:?}");
}
