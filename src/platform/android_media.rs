//! Android MediaSession access. Notification content is never read here.
use super::{java_text, service, AndroidPlatform};
use crate::{
    media::supports,
    model::{MediaAction, MediaSession, Playback},
};
use anyhow::{Context, Result};
use jni::{
    objects::{GlobalRef, JObject, JValue},
    JNIEnv,
};

struct Entry {
    id: String,
    token: GlobalRef,
    controller: GlobalRef,
}
#[derive(Default)]
pub(super) struct State {
    entries: Vec<Entry>,
    next: u64,
}
const LISTENER: &str = "dev.quiet.launcher.MediaAccessService";

fn read(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    state: &mut State,
) -> Result<Vec<MediaSession>> {
    let package = env
        .call_method(activity, "getPackageName", "()Ljava/lang/String;", &[])?
        .l()?;
    let class = env.new_string(LISTENER)?;
    let component = env.new_object(
        "android/content/ComponentName",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&package), JValue::Object(&class)],
    )?;
    let notification = service(env, activity, "notification")?;
    anyhow::ensure!(
        env.call_method(
            notification,
            "isNotificationListenerAccessGranted",
            "(Landroid/content/ComponentName;)Z",
            &[JValue::Object(&component)]
        )?
        .z()?,
        "通知アクセスが許可されていません"
    );
    let manager = service(env, activity, "media_session")?;
    let list = env
        .call_method(
            manager,
            "getActiveSessions",
            "(Landroid/content/ComponentName;)Ljava/util/List;",
            &[JValue::Object(&component)],
        )?
        .l()?;
    let pm = env
        .call_method(
            activity,
            "getPackageManager",
            "()Landroid/content/pm/PackageManager;",
            &[],
        )?
        .l()?;
    let count = env.call_method(&list, "size", "()I", &[])?.i()?;
    let mut entries = Vec::new();
    let mut sessions = Vec::new();
    for index in 0..count {
        env.with_local_frame(32, |env| -> Result<()> {
            let controller = env
                .call_method(&list, "get", "(I)Ljava/lang/Object;", &[JValue::Int(index)])?
                .l()?;
            let token = env
                .call_method(
                    &controller,
                    "getSessionToken",
                    "()Landroid/media/session/MediaSession$Token;",
                    &[],
                )?
                .l()?;
            let mut id = None;
            for old in &state.entries {
                if env
                    .call_method(
                        &token,
                        "equals",
                        "(Ljava/lang/Object;)Z",
                        &[JValue::Object(old.token.as_obj())],
                    )?
                    .z()?
                {
                    id = Some(old.id.clone());
                    break;
                }
            }
            let id = id.unwrap_or_else(|| {
                state.next += 1;
                format!("session:{}", state.next)
            });
            let pkg = env
                .call_method(&controller, "getPackageName", "()Ljava/lang/String;", &[])?
                .l()?;
            let package = java_text(env, &pkg)?;
            let info = env
                .call_method(
                    &pm,
                    "getApplicationInfo",
                    "(Ljava/lang/String;I)Landroid/content/pm/ApplicationInfo;",
                    &[JValue::Object(&pkg), JValue::Int(0)],
                )?
                .l()?;
            let label = env
                .call_method(
                    &pm,
                    "getApplicationLabel",
                    "(Landroid/content/pm/ApplicationInfo;)Ljava/lang/CharSequence;",
                    &[JValue::Object(&info)],
                )?
                .l()?;
            let mut provider = java_text(env, &label)?;
            let launch = env
                .call_method(
                    &pm,
                    "getLaunchIntentForPackage",
                    "(Ljava/lang/String;)Landroid/content/Intent;",
                    &[JValue::Object(&pkg)],
                )?
                .l()?;
            if !launch.is_null() {
                let resolved = env
                    .call_method(
                        &pm,
                        "resolveActivity",
                        "(Landroid/content/Intent;I)Landroid/content/pm/ResolveInfo;",
                        &[JValue::Object(&launch), JValue::Int(0)],
                    )?
                    .l()?;
                if !resolved.is_null() {
                    let label = env
                        .call_method(
                            resolved,
                            "loadLabel",
                            "(Landroid/content/pm/PackageManager;)Ljava/lang/CharSequence;",
                            &[JValue::Object(&pm)],
                        )?
                        .l()?;
                    let label = java_text(env, &label)?;
                    if !label.trim().is_empty() {
                        provider = label;
                    }
                }
            }
            let playback = env
                .call_method(
                    &controller,
                    "getPlaybackState",
                    "()Landroid/media/session/PlaybackState;",
                    &[],
                )?
                .l()?;
            let (status, actions) = if playback.is_null() {
                (0, 0)
            } else {
                (
                    env.call_method(&playback, "getState", "()I", &[])?.i()?,
                    env.call_method(&playback, "getActions", "()J", &[])?.j()?,
                )
            };
            let playback = match status {
                1 => Playback::Stopped,
                2 => Playback::Paused,
                3 => Playback::Playing,
                6 => Playback::Buffering,
                7 => Playback::Error,
                8 => Playback::Connecting,
                _ => Playback::None,
            };
            let metadata = env
                .call_method(
                    &controller,
                    "getMetadata",
                    "()Landroid/media/MediaMetadata;",
                    &[],
                )?
                .l()?;
            let mut title = None;
            if !metadata.is_null() {
                for key in [
                    "android.media.metadata.DISPLAY_TITLE",
                    "android.media.metadata.TITLE",
                ] {
                    let key = env.new_string(key)?;
                    let text = env
                        .call_method(
                            &metadata,
                            "getText",
                            "(Ljava/lang/String;)Ljava/lang/CharSequence;",
                            &[JValue::Object(&key)],
                        )?
                        .l()?;
                    let text = java_text(env, &text)?;
                    if !text.trim().is_empty() {
                        title = Some(text);
                        break;
                    }
                }
            }
            sessions.push(MediaSession {
                id: id.clone(),
                package,
                provider,
                title,
                state: playback,
                previous: actions & 16 != 0,
                next: actions & 32 != 0,
                play: actions & 4 != 0,
                pause: actions & 2 != 0,
            });
            entries.push(Entry {
                id,
                token: env.new_global_ref(token)?,
                controller: env.new_global_ref(controller)?,
            });
            Ok(())
        })?;
    }
    state.entries = entries;
    Ok(sessions)
}
pub(super) fn sessions(platform: &AndroidPlatform) -> Result<Vec<MediaSession>> {
    let result =
        platform.jni(|env, activity| read(env, activity, &mut platform.media.lock().unwrap()));
    if result.is_err() {
        platform.media.lock().unwrap().entries.clear();
    }
    result
}
pub(super) fn control(platform: &AndroidPlatform, id: &str, action: MediaAction) -> Result<()> {
    platform.jni(|env, activity| {
        let mut state = platform.media.lock().unwrap();
        let sessions = read(env, activity, &mut state)?;
        let session = sessions
            .iter()
            .find(|s| s.id == id)
            .context("再生対象が見つかりません")?;
        anyhow::ensure!(
            matches!(
                session.state,
                Playback::Playing | Playback::Paused | Playback::Buffering | Playback::Connecting
            ) && supports(session, action),
            "この操作は利用できません"
        );
        let controller = &state
            .entries
            .iter()
            .find(|e| e.id == id)
            .context("再生対象が見つかりません")?
            .controller;
        let controls = env
            .call_method(
                controller.as_obj(),
                "getTransportControls",
                "()Landroid/media/session/MediaController$TransportControls;",
                &[],
            )?
            .l()?;
        let method = match action {
            MediaAction::Previous => "skipToPrevious",
            MediaAction::Next => "skipToNext",
            MediaAction::Play => "play",
            MediaAction::Pause => "pause",
        };
        env.call_method(controls, method, "()V", &[])?;
        Ok(())
    })
}
