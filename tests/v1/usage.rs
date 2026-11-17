use quiet_launcher::usage::{screen_seconds, ScreenEvent::*};
const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
// A26: catches wrong clipping, duplicate addition, unsorted-event omission and Reset inflation.
#[test]
fn midnight_clips_previous_days_session() {
    assert_eq!(
        screen_seconds(&[On(-10 * MIN), Off(10 * MIN)], 0, 9 * HOUR),
        600
    );
}
#[test]
fn disjoint_known_intervals_are_summed() {
    assert_eq!(
        screen_seconds(
            &[
                On(8 * HOUR),
                Off(8 * HOUR + 20 * MIN),
                On(8 * HOUR + 30 * MIN),
                Off(8 * HOUR + 40 * MIN)
            ],
            0,
            9 * HOUR
        ),
        1800
    );
}
#[test]
fn open_known_session_is_counted_to_now() {
    assert_eq!(screen_seconds(&[On(8 * HOUR + 50 * MIN)], 0, 9 * HOUR), 600);
}
#[test]
fn seconds_are_summed_before_rounding_to_minutes() {
    assert_eq!(
        screen_seconds(&[On(0), Off(40_000), On(60_000), Off(100_000)], 0, 120_000),
        80
    );
}
#[test]
fn duplicate_boundaries_do_not_double_count() {
    assert_eq!(
        screen_seconds(
            &[On(0), On(0), On(10_000), Off(40_000), Off(40_000)],
            0,
            60_000
        ),
        40
    );
}
#[test]
fn shutdown_without_a_reliable_end_does_not_invent_time() {
    assert_eq!(
        screen_seconds(&[On(0), Reset(40_000), On(60_000), Off(80_000)], 0, 100_000),
        20
    );
}
#[test]
fn out_of_order_delivery_is_stably_sorted_before_aggregation() {
    assert_eq!(
        screen_seconds(&[Off(20 * MIN), On(10 * MIN)], 0, 30 * MIN),
        600
    );
}
#[test]
fn future_records_do_not_hide_earlier_valid_records() {
    assert_eq!(
        screen_seconds(&[On(40 * MIN), On(10 * MIN), Off(20 * MIN)], 0, 30 * MIN),
        600
    );
}
#[test]
fn zero_and_negative_ranges_do_not_invent_time() {
    assert_eq!(screen_seconds(&[On(0)], 10_000, 10_000), 0);
    assert_eq!(screen_seconds(&[On(0)], 20_000, 10_000), 0);
}
#[test]
fn rolling_twenty_four_hours_is_not_the_local_calendar_day() {
    let events = [On(0), Off(8 * HOUR), On(28 * HOUR)];
    assert_eq!(screen_seconds(&events, 6 * HOUR, 30 * HOUR), 14_400);
    assert_eq!(screen_seconds(&events, 24 * HOUR, 30 * HOUR), 7_200);
}
