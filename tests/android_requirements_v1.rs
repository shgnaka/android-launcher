//! Real Android acceptance. Missing tools/devices are errors, never automatic successes.
#[path = "support/android.rs"]
mod device;
use anyhow::Result;
use device::{settings, Android};
#[test]
fn a02_home_while_searching_resets_to_the_launcher_home() -> Result<()> {
    let lab = Android::new("A02-home", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera")?;
    lab.wait_text("Camera")?;
    lab.shell(&[
        "am",
        "start",
        "-W",
        "-a",
        "android.intent.action.MAIN",
        "-c",
        "android.intent.category.HOME",
        "-n",
        "dev.quiet.launcher/android.app.NativeActivity",
    ])?;
    lab.wait_text("メモ")?;
    lab.foreground("dev.quiet.launcher")
}
#[test]
fn a11_one_back_from_the_real_ime_returns_home_and_clears_search() -> Result<()> {
    let lab = Android::new("A11-ime-back", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera")?;
    lab.wait_text("Camera")?;
    lab.back()?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.wait_text("Camera Tools")?;
    Ok(())
}
#[test]
fn a05_same_named_apps_have_distinct_identity_and_launch_destinations() -> Result<()> {
    let lab = Android::new("A05-identities", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera")?;
    lab.wait_text("example.camera")?;
    lab.wait_text("example.other")?;
    lab.tap_text("example.other")?;
    lab.dialog_text("FIXTURE OTHER MAIN")?;
    lab.foreground("example.other")
}
#[test]
fn a05_second_activity_of_the_same_package_remains_launchable() -> Result<()> {
    let lab = Android::new("A05-second", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera%sTools")?;
    lab.wait_text("Camera Tools")?;
    lab.tap_app("Camera Tools")?;
    lab.dialog_text("FIXTURE CAMERA TOOLS")?;
    lab.foreground("example.camera")
}
#[test]
fn a08_home_name_long_press_does_not_open_an_operation_menu() -> Result<()> {
    let lab = Android::new("A08-home-long", settings())?;
    lab.wait_text("メモ")?;
    lab.long_press("メモ")?;
    lab.assert_absent("アプリを非表示")?;
    lab.assert_absent("文字サイズ")?;
    lab.foreground("dev.quiet.launcher")
}
#[test]
fn a17_real_drawer_long_press_opens_actions_without_launching() -> Result<()> {
    let lab = Android::new("A17-menu", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera%sTools")?;
    lab.long_press_app("Camera Tools")?;
    lab.wait_text("アプリを非表示")?;
    lab.foreground("dev.quiet.launcher")?;
    anyhow::ensure!(
        lab.saved()?["hidden_packages"]
            .as_array()
            .unwrap()
            .is_empty(),
        "opening the menu must not hide"
    );
    Ok(())
}
#[test]
fn a16_hiding_an_activity_hides_its_whole_package() -> Result<()> {
    let lab = Android::new("A16-hide", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera%sTools")?;
    lab.long_press_app("Camera Tools")?;
    lab.tap_text("アプリを非表示")?;
    anyhow::ensure!(
        lab.saved()?["hidden_packages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "example.camera"),
        "package hide must persist"
    );
    lab.restart()?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera%sTools")?;
    lab.wait_text("一致するアプリはありません")?;
    Ok(())
}
#[test]
fn a39_romaji_matches_the_actual_camera_name() -> Result<()> {
    let lab = Android::new("A39-reading", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("kamera")?;
    lab.wait_text("Camera")?;
    lab.assert_absent("Camera Tools")?;
    Ok(())
}
#[test]
fn a19_hidden_package_is_managed_only_in_the_dedicated_section() -> Result<()> {
    let mut s = settings();
    s["hidden_packages"] = serde_json::json!(["example.camera"]);
    let lab = Android::new("A19-hidden-settings", s)?;
    lab.wait_text("メモ")?;
    lab.settings()?;
    lab.scroll_to("非表示アプリ")?;
    lab.scroll_to("再表示")?;
    lab.wait_text("Camera")?;
    lab.tap_text("再表示")?;
    anyhow::ensure!(
        lab.saved()?["hidden_packages"]
            .as_array()
            .unwrap()
            .is_empty(),
        "unhide must persist"
    );
    lab.back()?;
    lab.wait_text("メモ")?;
    Ok(())
}
#[test]
fn a27_real_media_sessions_have_a_visible_selected_target() -> Result<()> {
    let mut s = settings();
    s["show_media"] = true.into();
    let mut lab = Android::new("A27-media", s)?;
    lab.fixture("example.player1", "Main")?;
    lab.dialog_text("PLAYER ONE READY")?;
    lab.fixture("example.player2", "Main")?;
    lab.dialog_text("PLAYER TWO READY")?;
    lab.enable_listener()?;
    lab.start()?;
    lab.wait_text("PLAYER ONE")?;
    lab.wait_text("TRACK B")?;
    lab.assert_absent("TRACK X")?;
    Ok(())
}
#[test]
fn a41_horizontal_media_switch_sends_no_transport_command() -> Result<()> {
    let mut s = settings();
    s["show_media"] = true.into();
    let mut lab = Android::new("A41-media-switch", s)?;
    lab.fixture("example.player1", "Main")?;
    lab.dialog_text("PLAYER ONE READY")?;
    lab.fixture("example.player2", "Main")?;
    lab.dialog_text("PLAYER TWO READY")?;
    lab.enable_listener()?;
    lab.start()?;
    let b = lab.wait_text("PLAYER ONE")?;
    let y = ((b[1] + b[3]) / 2).to_string();
    lab.shell(&[
        "input",
        "swipe",
        &(lab.width * 4 / 5).to_string(),
        &y,
        &(lab.width / 5).to_string(),
        &y,
        "350",
    ])?;
    lab.wait_text("PLAYER TWO")?;
    lab.wait_text("TRACK X")?;
    anyhow::ensure!(lab.session("example.player1")?["events"]
        .as_array()
        .unwrap()
        .is_empty());
    anyhow::ensure!(lab.session("example.player2")?["events"]
        .as_array()
        .unwrap()
        .is_empty());
    Ok(())
}
#[test]
fn a03_process_restart_preserves_favorites_and_hide_preferences() -> Result<()> {
    let mut s = settings();
    s["hidden_packages"] = serde_json::json!(["example.camera"]);
    let lab = Android::new("A03-restart", s)?;
    lab.wait_text("メモ")?;
    lab.restart()?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera%sTools")?;
    lab.wait_text("一致するアプリはありません")?;
    Ok(())
}
#[test]
fn a35_native_input_and_results_are_above_the_ime() -> Result<()> {
    let lab = Android::new("A35-ime", settings())?;
    lab.wait_text("メモ")?;
    lab.drawer()?;
    lab.type_text("Camera%sTools")?;
    let b = lab.app_bounds("Camera Tools")?;
    anyhow::ensure!(
        b[1] >= 0 && b[3] < lab.height / 2,
        "first result must remain above the visible IME"
    );
    let xml =
        String::from_utf8(lab.shell(&["uiautomator", "dump", "/data/local/tmp/quiet-v1.xml"])?)?;
    anyhow::ensure!(xml.contains("dumped"));
    let hierarchy = String::from_utf8(lab.shell(&["cat", "/data/local/tmp/quiet-v1.xml"])?)?;
    std::fs::write(lab.out.join("input.xml"), &hierarchy)?;
    anyhow::ensure!(
        hierarchy.contains("android.widget.EditText") && hierarchy.contains("Camera Tools"),
        "real native inline field must expose the input"
    );
    Ok(())
}
#[test]
fn a45_corrupt_settings_never_publish_the_cached_app_list() -> Result<()> {
    let lab = Android::new("A45-corrupt", settings())?;
    lab.wait_text("メモ")?;
    lab.shell(&["am", "force-stop", "dev.quiet.launcher"])?;
    lab.put_settings(b"corrupt privacy settings")?;
    lab.start()?;
    lab.wait_text("設定を読み取れません")?;
    lab.drawer()?;
    lab.assert_absent("Camera Tools")?;
    Ok(())
}
#[test]
fn a30_content_description_is_displayed_when_text_is_empty() -> Result<()> {
    let lab = Android::new("A30-description", settings())?;
    lab.add_widget("description", "WIDGET DESC")?;
    lab.back()?;
    lab.wait_text("WIDGET DESC")?;
    lab.wait_text("DESCRIPTION READY")?;
    Ok(())
}
#[test]
fn a30_image_only_content_has_an_explicit_unavailable_error() -> Result<()> {
    let lab = Android::new("A30-image", settings())?;
    lab.add_widget("image", "WIDGET IMAGE")?;
    lab.back()?;
    lab.wait_text("WIDGET IMAGE")?;
    lab.wait_text("内容を表示できません")?;
    Ok(())
}
#[test]
fn a30_gone_views_never_leak_hidden_text_into_home_information() -> Result<()> {
    let lab = Android::new("A30-hidden-view", settings())?;
    lab.add_widget("hidden", "WIDGET HIDDEN")?;
    lab.back()?;
    lab.wait_text("PUBLIC READY")?;
    lab.assert_absent("SECRET VALUE")?;
    Ok(())
}
#[test]
fn a30_duplicate_text_and_description_are_shown_once() -> Result<()> {
    let lab = Android::new("A30-duplicates", settings())?;
    lab.add_widget("duplicates", "WIDGET DUP")?;
    lab.back()?;
    lab.wait_text("ONLY ONCE")?;
    let count = lab
        .lines("deduplicated.png")?
        .iter()
        .map(|(s, _)| s.replace(' ', "").matches("ONLYONCE").count())
        .sum::<usize>();
    anyhow::ensure!(count == 1, "duplicate widget text rendered {count} times");
    Ok(())
}
#[test]
fn a29_cancelled_configuration_does_not_leave_a_registered_widget() -> Result<()> {
    let lab = Android::new("A29-config-cancel", settings())?;
    lab.add_widget("configuration", "WIDGET CONFIG")?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    loop {
        if lab.foreground("example.widget.configuration").is_ok() {
            break;
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "configuration activity was not launched"
        );
        std::thread::yield_now();
    }
    lab.back()?;
    lab.wait_text("メモ")?;
    anyhow::ensure!(
        lab.saved()?["widget_ids"].as_array().unwrap().is_empty(),
        "cancellation must discard the temporary widget registration"
    );
    Ok(())
}
