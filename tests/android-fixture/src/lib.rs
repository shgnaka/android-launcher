//! Real Android MediaSession callbacks and visible activity identities; no Java/Kotlin sources.
use jni::{
    objects::{GlobalRef, JObject, JString, JValue},
    JNIEnv, JavaVM, NativeMethod,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};
static PLAYER: OnceLock<Mutex<Player>> = OnceLock::new();
struct Player {
    session: GlobalRef,
    index: usize,
    playing: bool,
    second: bool,
    path: PathBuf,
    events: Vec<String>,
}
fn text(env: &mut JNIEnv<'_>, o: JObject<'_>) -> String {
    env.get_string(&JString::from(o)).unwrap().into()
}
impl Player {
    fn publish(&self, env: &mut JNIEnv<'_>) -> jni::errors::Result<()> {
        let tracks = if self.second {
            &["TRACK X", "TRACK Y"][..]
        } else {
            &["TRACK A", "TRACK B", "TRACK C"][..]
        };
        let builder = env.new_object("android/media/MediaMetadata$Builder", "()V", &[])?;
        let key = env.new_string("android.media.metadata.TITLE")?;
        let value = env.new_string(tracks[self.index])?;
        env.call_method(
            &builder,
            "putString",
            "(Ljava/lang/String;Ljava/lang/String;)Landroid/media/MediaMetadata$Builder;",
            &[JValue::Object(&key), JValue::Object(&value)],
        )?;
        let metadata = env
            .call_method(builder, "build", "()Landroid/media/MediaMetadata;", &[])?
            .l()?;
        env.call_method(
            self.session.as_obj(),
            "setMetadata",
            "(Landroid/media/MediaMetadata;)V",
            &[JValue::Object(&metadata)],
        )?;
        let state = env.new_object("android/media/session/PlaybackState$Builder", "()V", &[])?;
        env.call_method(
            &state,
            "setActions",
            "(J)Landroid/media/session/PlaybackState$Builder;",
            &[JValue::Long(if self.second { 38 } else { 54 })],
        )?;
        env.call_method(
            &state,
            "setState",
            "(IJF)Landroid/media/session/PlaybackState$Builder;",
            &[
                JValue::Int(if self.playing { 3 } else { 2 }),
                JValue::Long(0),
                JValue::Float(if self.playing { 1. } else { 0. }),
            ],
        )?;
        let state = env
            .call_method(
                state,
                "build",
                "()Landroid/media/session/PlaybackState;",
                &[],
            )?
            .l()?;
        env.call_method(
            self.session.as_obj(),
            "setPlaybackState",
            "(Landroid/media/session/PlaybackState;)V",
            &[JValue::Object(&state)],
        )?;
        let data = serde_json::json!({"track":tracks[self.index],"playing":self.playing,"events":self.events});
        std::fs::write(&self.path, serde_json::to_vec(&data).unwrap()).unwrap();
        Ok(())
    }
}
fn command(env: &mut JNIEnv<'_>, name: &str) {
    let mut p = PLAYER.get().unwrap().lock().unwrap();
    p.events.push(name.into());
    match name {
        "play" => p.playing = true,
        "pause" => p.playing = false,
        "next" => p.index = (p.index + 1).min(if p.second { 1 } else { 2 }),
        "previous" => p.index = p.index.saturating_sub(1),
        _ => {}
    };
    if let Err(e) = p.publish(env) {
        log::error!("fixture callback: {e}");
    }
}
extern "system" fn play(mut e: JNIEnv<'_>, _: JObject<'_>) {
    command(&mut e, "play")
}
extern "system" fn pause(mut e: JNIEnv<'_>, _: JObject<'_>) {
    command(&mut e, "pause")
}
extern "system" fn next(mut e: JNIEnv<'_>, _: JObject<'_>) {
    command(&mut e, "next")
}
extern "system" fn previous(mut e: JNIEnv<'_>, _: JObject<'_>) {
    command(&mut e, "previous")
}
fn initialize(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    path: PathBuf,
) -> jni::errors::Result<()> {
    let package_o = env
        .call_method(activity, "getPackageName", "()Ljava/lang/String;", &[])?
        .l()?;
    let package = text(env, package_o);
    let component = env
        .call_method(
            activity,
            "getComponentName",
            "()Landroid/content/ComponentName;",
            &[],
        )?
        .l()?;
    let class_o = env
        .call_method(component, "getClassName", "()Ljava/lang/String;", &[])?
        .l()?;
    let class = text(env, class_o);
    let message = if package == "example.camera" {
        if class.ends_with("Tools") {
            "FIXTURE CAMERA TOOLS"
        } else {
            "FIXTURE CAMERA MAIN"
        }
    } else if package == "example.other" {
        "FIXTURE OTHER MAIN"
    } else if package == "example.notes" {
        "FIXTURE NOTES MAIN"
    } else if package == "example.player2" {
        "PLAYER TWO READY"
    } else {
        "PLAYER ONE READY"
    };
    if package.starts_with("example.player") {
        let second = package.ends_with('2');
        let bytes = env.byte_array_from_slice(include_bytes!(
            "../../../target/acceptance-fixtures/dex/classes.dex"
        ))?;
        let buffer = env
            .call_static_method(
                "java/nio/ByteBuffer",
                "wrap",
                "([B)Ljava/nio/ByteBuffer;",
                &[JValue::Object(&bytes)],
            )?
            .l()?;
        let parent = env
            .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
            .l()?;
        let loader = env.new_object(
            "dalvik/system/InMemoryDexClassLoader",
            "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
            &[JValue::Object(&buffer), JValue::Object(&parent)],
        )?;
        let name = env.new_string("dev.quiet.fixture.MediaCallbacks")?;
        let class = env
            .call_method(
                loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name)],
            )?
            .l()?;
        let class = jni::objects::JClass::from(class);
        env.register_native_methods(
            &class,
            &[
                NativeMethod {
                    name: "onPlay".into(),
                    sig: "()V".into(),
                    fn_ptr: play as *mut _,
                },
                NativeMethod {
                    name: "onPause".into(),
                    sig: "()V".into(),
                    fn_ptr: pause as *mut _,
                },
                NativeMethod {
                    name: "onSkipToNext".into(),
                    sig: "()V".into(),
                    fn_ptr: next as *mut _,
                },
                NativeMethod {
                    name: "onSkipToPrevious".into(),
                    sig: "()V".into(),
                    fn_ptr: previous as *mut _,
                },
            ],
        )?;
        let callback = env.new_object(&class, "()V", &[])?;
        let tag = env.new_string(if second { "QUIET-P2" } else { "QUIET-P1" })?;
        let session = env.new_object(
            "android/media/session/MediaSession",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(activity), JValue::Object(&tag)],
        )?;
        env.call_method(
            &session,
            "setCallback",
            "(Landroid/media/session/MediaSession$Callback;)V",
            &[JValue::Object(&callback)],
        )?;
        let p = Player {
            session: env.new_global_ref(&session)?,
            index: if second { 0 } else { 1 },
            playing: !second,
            second,
            path: path.join("session.json"),
            events: vec![],
        };
        p.publish(env)?;
        let _ = PLAYER.set(Mutex::new(p));
        env.call_method(session, "setActive", "(Z)V", &[JValue::Bool(1)])?;
    }
    let builder = env.new_object(
        "android/app/AlertDialog$Builder",
        "(Landroid/content/Context;)V",
        &[JValue::Object(activity)],
    )?;
    let message = env.new_string(message)?;
    env.call_method(
        &builder,
        "setMessage",
        "(Ljava/lang/CharSequence;)Landroid/app/AlertDialog$Builder;",
        &[JValue::Object(&message)],
    )?;
    env.call_method(
        &builder,
        "setCancelable",
        "(Z)Landroid/app/AlertDialog$Builder;",
        &[JValue::Bool(0)],
    )?;
    env.call_method(builder, "show", "()Landroid/app/AlertDialog;", &[])?;
    Ok(())
}
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    let vm = Arc::new(unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.unwrap());
    let activity = {
        let env = vm.attach_current_thread().unwrap();
        env.new_global_ref(unsafe { JObject::from_raw(app.activity_as_ptr().cast()) })
            .unwrap()
    };
    let path = app.internal_data_path().unwrap();
    let _ = std::fs::write(path.join("entry.txt"), "android_main entered");
    let mut initialized = false;
    let mut destroyed = false;
    while !destroyed {
        app.poll_events(Some(std::time::Duration::from_millis(100)), |event| {
            if let android_activity::PollEvent::Main(main) = event {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path.join("entry.txt"))
                {
                    let _ = writeln!(f, "{main:?}");
                }
                // NativeActivity must present a first buffer before Android removes its
                // launch splash; otherwise a correctly created dialog remains covered.
                if matches!(
                    main,
                    android_activity::MainEvent::InitWindow { .. }
                        | android_activity::MainEvent::RedrawNeeded { .. }
                        | android_activity::MainEvent::WindowResized { .. }
                ) {
                    if let Some(window) = app.native_window() {
                        if let Ok(mut buffer) = window.lock(None) {
                            if let Some(bytes) = buffer.bytes() {
                                for byte in bytes {
                                    byte.write(244);
                                }
                            }
                        }
                    }
                }
                if matches!(main, android_activity::MainEvent::InputAvailable) {
                    if let Ok(mut events) = app.input_events_iter() {
                        while events.next(|_| android_activity::InputStatus::Unhandled) {}
                    }
                }
                match main {
                    android_activity::MainEvent::InitWindow { .. }
                    | android_activity::MainEvent::GainedFocus
                        if !initialized =>
                    {
                        initialized = true;
                        let vm = vm.clone();
                        let activity = activity.clone();
                        let path = path.clone();
                        app.run_on_java_main_thread(Box::new(move || {
                            let mut env = vm.attach_current_thread().unwrap();
                            let _ = std::fs::write(
                                path.join("initializing.txt"),
                                "java main closure entered",
                            );
                            if let Err(e) = initialize(&mut env, activity.as_obj(), path.clone()) {
                                let _ = env.exception_describe();
                                let _ = env.exception_clear();
                                let _ = std::fs::write(path.join("error.txt"), format!("{e}"));
                                log::error!("fixture initialization: {e}");
                            }
                        }));
                    }
                    android_activity::MainEvent::Destroy => destroyed = true,
                    _ => {}
                }
            }
        });
    }
}
