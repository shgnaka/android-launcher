//! Measurement-tool tests, not launcher feature tests.
#[path = "support/ocr_v1_revision.rs"]
mod observer;
const SAME_LINE:&str="5\t1\t11\t1\t1\t1\t44\t594\t106\t24\t95\tCamera\n5\t1\t11\t1\t1\t2\t179\t594\t28\t28\t90\t再\n5\t1\t11\t1\t1\t3\t218\t592\t52\t30\t83\t表示\n";
#[test]
fn adjacent_label_does_not_move_the_button_tap_to_the_gap() {
    assert_eq!(
        observer::control_bounds(SAME_LINE, "再表示").unwrap(),
        [179, 592, 270, 622]
    );
}
#[test]
fn label_can_still_be_selected_by_its_own_word() {
    assert_eq!(
        observer::control_bounds(SAME_LINE, "Camera").unwrap(),
        [44, 594, 150, 618]
    );
}
#[test]
fn missing_control_is_an_observation_error() {
    assert!(observer::control_bounds(SAME_LINE, "削除").is_err());
}
#[test]
fn ambiguous_controls_are_not_silently_tapped() {
    let second = SAME_LINE.replace("\t11\t", "\t12\t");
    assert!(observer::control_bounds(&(SAME_LINE.to_owned() + &second), "再表示").is_err());
}
#[test]
fn tokens_on_different_lines_do_not_form_a_control() {
    let split = SAME_LINE.replace("1\t1\t3\t218", "1\t2\t3\t218");
    assert!(observer::control_bounds(&split, "再表示").is_err());
}
#[test]
fn mixed_token_requires_better_observation_instead_of_guessing() {
    assert!(observer::control_bounds(
        "5\t1\t1\t1\t1\t1\t44\t594\t226\t30\t95\tCamera再表示\n",
        "再表示"
    )
    .is_err());
}
