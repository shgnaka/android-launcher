//! Android framework calls only. No Java or Kotlin application sources are used.
use super::*;
use crate::usage::{self, ScreenEvent};
use anyhow::{anyhow, Context};
use chrono::{Local, TimeZone};
use jni::{
    objects::{GlobalRef, JObject, JString, JValue},
    JNIEnv, JavaVM,
};
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub type MainThread = Arc<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>;
pub type WindowInsets = Arc<dyn Fn() -> [f32; 4] + Send + Sync>;
#[path = "android_media.rs"]
mod media_bridge;
#[path = "android_widget_results.rs"]
mod widget_results;
const HOST_ID: i32 = 0x51554945;

#[derive(Default)]
struct JavaState {
    host: Option<GlobalRef>,
    views: HashMap<i32, GlobalRef>,
    configure_pending: HashSet<i32>,
    widgets: Vec<TextWidget>,
    prompt: Option<(String, GlobalRef, GlobalRef)>,
    text_result: Option<(String, String)>,
    search_edit: Option<GlobalRef>,
    search_visible: bool,
    search_popup: Option<GlobalRef>,
    search_editing: bool,
    search_had_ime: bool,
    search_focus_attempts: u8,
    search_retry_at: Option<Instant>,
    search_text: String,
    error: Option<String>,
}

#[derive(Clone)]
pub struct AndroidPlatform {
    media: Arc<Mutex<media_bridge::State>>,
    vm: Arc<JavaVM>,
    activity: GlobalRef,
    main: MainThread,
    state: Arc<Mutex<JavaState>>,
    insets: WindowInsets,
    system_insets: Arc<[AtomicI32; 4]>,
    dark: Arc<AtomicBool>,
    ime_bottom: Arc<AtomicI32>,
}

impl AndroidPlatform {
    /// # Safety
    /// `vm` and `activity` must be live JNI references supplied by AndroidApp.
    pub unsafe fn new(
        vm: *mut c_void,
        activity: *mut c_void,
        main: MainThread,
        insets: WindowInsets,
    ) -> Result<Self> {
        let vm = Arc::new(unsafe { JavaVM::from_raw(vm.cast()) }?);
        let activity = {
            let env = vm.attach_current_thread()?;
            // JObject is a borrowed wrapper: dropping it does not delete Android's reference.
            env.new_global_ref(unsafe { JObject::from_raw(activity.cast()) })?
        };
        Ok(Self {
            media: Arc::new(Mutex::new(media_bridge::State::default())),
            vm,
            activity,
            main,
            state: Arc::new(Mutex::new(JavaState::default())),
            insets,
            system_insets: Arc::new(std::array::from_fn(|_| AtomicI32::new(0))),
            dark: Arc::new(AtomicBool::new(false)),
            ime_bottom: Arc::new(AtomicI32::new(0)),
        })
    }

    fn jni<T>(&self, f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> Result<T>) -> Result<T> {
        let mut env = self.vm.attach_current_thread()?;
        let result = env.with_local_frame(128, |env| f(env, self.activity.as_obj()));
        if env.exception_check().unwrap_or(false) {
            // Clear exceptions before reusing this attached JVM thread.
            let exception = env.exception_occurred()?;
            env.exception_clear()?;
            let message = java_text(&mut env, exception.as_ref())
                .unwrap_or_else(|_| "Android exception".into());
            let _ = env.exception_clear();
            return Err(anyhow!("{message}"));
        }
        result
    }

    fn on_main<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>, &mut JavaState) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (tx, rx) = mpsc::sync_channel(1);
        let this = self.clone();
        (self.main)(Box::new(move || {
            let result =
                this.jni(|env, activity| f(env, activity, &mut this.state.lock().unwrap()));
            let _ = tx.send(result);
        }));
        rx.recv_timeout(Duration::from_secs(5))
            .context("Android の応答を待っています")?
    }
}

fn java_text(env: &mut JNIEnv<'_>, value: &JObject<'_>) -> Result<String> {
    if value.is_null() {
        return Ok(String::new());
    }
    let string = env
        .call_method(value, "toString", "()Ljava/lang/String;", &[])?
        .l()?;
    Ok(env.get_string(&JString::from(string))?.into())
}

fn service<'a>(env: &mut JNIEnv<'a>, activity: &JObject<'_>, name: &str) -> Result<JObject<'a>> {
    let name = env.new_string(name)?;
    Ok(env
        .call_method(
            activity,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&name)],
        )?
        .l()?)
}

fn intent<'a>(env: &mut JNIEnv<'a>, action: &str) -> Result<JObject<'a>> {
    let action = env.new_string(action)?;
    Ok(env.new_object(
        "android/content/Intent",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&action)],
    )?)
}

fn start(env: &mut JNIEnv<'_>, activity: &JObject<'_>, intent: &JObject<'_>) -> Result<()> {
    env.call_method(
        activity,
        "startActivity",
        "(Landroid/content/Intent;)V",
        &[JValue::Object(intent)],
    )?;
    Ok(())
}

fn start_widget_activity(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    intent: &JObject<'_>,
) -> Result<()> {
    let component = env.new_string("dev.quiet.launcher.WidgetRequestActivity")?;
    let request = env.new_object("android/content/Intent", "()V", &[])?;
    let package = env
        .call_method(activity, "getPackageName", "()Ljava/lang/String;", &[])?
        .l()?;
    env.call_method(
        &request,
        "setClassName",
        "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&package), JValue::Object(&component)],
    )?;
    let key = env.new_string("quiet.widget.request")?;
    env.call_method(
        &request,
        "putExtra",
        "(Ljava/lang/String;Landroid/os/Parcelable;)Landroid/content/Intent;",
        &[JValue::Object(&key), JValue::Object(intent)],
    )?;
    let key = env.new_string("appWidgetId")?;
    let id = env
        .call_method(
            intent,
            "getIntExtra",
            "(Ljava/lang/String;I)I",
            &[JValue::Object(&key), JValue::Int(-1)],
        )?
        .i()?;
    extra_id(env, &request, id)?;
    start(env, activity, &request)
}

fn component<'a>(env: &mut JNIEnv<'a>, flat: &str) -> Result<JObject<'a>> {
    let flat = env.new_string(flat)?;
    let value = env
        .call_static_method(
            "android/content/ComponentName",
            "unflattenFromString",
            "(Ljava/lang/String;)Landroid/content/ComponentName;",
            &[JValue::Object(&flat)],
        )?
        .l()?;
    if value.is_null() {
        anyhow::bail!("アプリの識別子が不正です");
    }
    Ok(value)
}

fn extra_id(env: &mut JNIEnv<'_>, intent: &JObject<'_>, id: i32) -> Result<()> {
    let key = env.new_string("appWidgetId")?;
    env.call_method(
        intent,
        "putExtra",
        "(Ljava/lang/String;I)Landroid/content/Intent;",
        &[JValue::Object(&key), JValue::Int(id)],
    )?;
    Ok(())
}

fn manager<'a>(env: &mut JNIEnv<'a>, activity: &JObject<'_>) -> Result<JObject<'a>> {
    Ok(env
        .call_static_method(
            "android/appwidget/AppWidgetManager",
            "getInstance",
            "(Landroid/content/Context;)Landroid/appwidget/AppWidgetManager;",
            &[JValue::Object(activity)],
        )?
        .l()?)
}

fn ensure_host(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    state: &mut JavaState,
) -> Result<GlobalRef> {
    if state.host.is_none() {
        let host = env.new_object(
            "android/appwidget/AppWidgetHost",
            "(Landroid/content/Context;I)V",
            &[JValue::Object(activity), JValue::Int(HOST_ID)],
        )?;
        env.call_method(&host, "startListening", "()V", &[])?;
        state.host = Some(env.new_global_ref(host)?);
    }
    Ok(state.host.as_ref().unwrap().clone())
}

impl Platform for AndroidPlatform {
    fn catalog_revision(&self) -> Result<Option<String>> {
        self.jni(|env, activity| {
            let pm = env
                .call_method(
                    activity,
                    "getPackageManager",
                    "()Landroid/content/pm/PackageManager;",
                    &[],
                )?
                .l()?;
            let changed = env
                .call_method(
                    pm,
                    "getChangedPackages",
                    "(I)Landroid/content/pm/ChangedPackages;",
                    &[JValue::Int(0)],
                )?
                .l()?;
            let sequence = if changed.is_null() {
                0
            } else {
                env.call_method(changed, "getSequenceNumber", "()I", &[])?
                    .i()?
            };
            let resolver = env
                .call_method(
                    activity,
                    "getContentResolver",
                    "()Landroid/content/ContentResolver;",
                    &[],
                )?
                .l()?;
            let boot_key = env.new_string("boot_count")?;
            let boot = env
                .call_static_method(
                    "android/provider/Settings$Global",
                    "getInt",
                    "(Landroid/content/ContentResolver;Ljava/lang/String;I)I",
                    &[
                        JValue::Object(&resolver),
                        JValue::Object(&boot_key),
                        JValue::Int(0),
                    ],
                )?
                .i()?;
            let locale = env
                .call_static_method(
                    "java/util/Locale",
                    "getDefault",
                    "()Ljava/util/Locale;",
                    &[],
                )?
                .l()?;
            let tag = env
                .call_method(locale, "toLanguageTag", "()Ljava/lang/String;", &[])?
                .l()?;
            Ok(Some(format!("{boot}:{sequence}:{}", java_text(env, &tag)?)))
        })
    }
    fn is_android(&self) -> bool {
        true
    }

    fn apps(&self) -> Result<Vec<AppEntry>> {
        self.jni(|env, activity| {
            let pm = env
                .call_method(
                    activity,
                    "getPackageManager",
                    "()Landroid/content/pm/PackageManager;",
                    &[],
                )?
                .l()?;
            let query = intent(env, "android.intent.action.MAIN")?;
            let category = env.new_string("android.intent.category.LAUNCHER")?;
            env.call_method(
                &query,
                "addCategory",
                "(Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&category)],
            )?;
            let list = env
                .call_method(
                    &pm,
                    "queryIntentActivities",
                    "(Landroid/content/Intent;I)Ljava/util/List;",
                    &[JValue::Object(&query), JValue::Int(0)],
                )?
                .l()?;
            let length = env.call_method(&list, "size", "()I", &[])?.i()?;
            let mut apps = Vec::new();
            for index in 0..length {
                let entry = env.with_local_frame(32, |env| -> Result<AppEntry> {
                    let info = env
                        .call_method(&list, "get", "(I)Ljava/lang/Object;", &[JValue::Int(index)])?
                        .l()?;
                    let label = env
                        .call_method(
                            &info,
                            "loadLabel",
                            "(Landroid/content/pm/PackageManager;)Ljava/lang/CharSequence;",
                            &[JValue::Object(&pm)],
                        )?
                        .l()?;
                    let activity_info = env
                        .get_field(&info, "activityInfo", "Landroid/content/pm/ActivityInfo;")?
                        .l()?;
                    let package = env
                        .get_field(&activity_info, "packageName", "Ljava/lang/String;")?
                        .l()?;
                    let name = env
                        .get_field(&activity_info, "name", "Ljava/lang/String;")?
                        .l()?;
                    Ok(AppEntry {
                        id: format!("{}/{}", java_text(env, &package)?, java_text(env, &name)?),
                        label: java_text(env, &label)?,
                    })
                })?;
                if !entry.id.starts_with("dev.quiet.launcher/") {
                    apps.push(entry);
                }
            }
            apps.sort_by_cached_key(|app| (app.label.to_lowercase(), app.id.clone()));
            apps.dedup_by(|a, b| a.id == b.id);
            Ok(apps)
        })
    }

    fn launch(&self, id: &str) -> Result<()> {
        let id = id.to_owned();
        self.on_main(move |env, activity, _| {
            let intent = intent(env, "android.intent.action.MAIN")?;
            let target = component(env, &id)?;
            let category = env.new_string("android.intent.category.LAUNCHER")?;
            env.call_method(
                &intent,
                "addCategory",
                "(Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&category)],
            )?;
            env.call_method(
                &intent,
                "setComponent",
                "(Landroid/content/ComponentName;)Landroid/content/Intent;",
                &[JValue::Object(&target)],
            )?;
            env.call_method(
                &intent,
                "addFlags",
                "(I)Landroid/content/Intent;",
                &[JValue::Int(0x10200000)],
            )?;
            start(env, activity, &intent)
        })
    }

    fn settings(&self, settings: SystemSettings) -> Result<()> {
        self.on_main(move |env, activity, _| {
            let action = match settings {
                SystemSettings::Home => "android.settings.HOME_SETTINGS",
                SystemSettings::Usage => "android.settings.USAGE_ACCESS_SETTINGS",
            };
            let intent = intent(env, action)?;
            start(env, activity, &intent)
        })
    }

    fn snapshot(&self) -> Result<DeviceSnapshot> {
        self.snapshot_range(false)
    }

    fn snapshot_range(&self, last_24h: bool) -> Result<DeviceSnapshot> {
        self.jni(|env, activity| {
            let action = env.new_string("android.intent.action.BATTERY_CHANGED")?;
            let filter = env.new_object("android/content/IntentFilter", "(Ljava/lang/String;)V", &[JValue::Object(&action)])?;
            let battery = env.call_method(activity, "registerReceiver", "(Landroid/content/BroadcastReceiver;Landroid/content/IntentFilter;)Landroid/content/Intent;", &[JValue::Object(&JObject::null()), JValue::Object(&filter)])?.l()?;
            let mut battery_percent = None;
            if !battery.is_null() {
                let level_key = env.new_string("level")?;
                let scale_key = env.new_string("scale")?;
                let level = env.call_method(&battery, "getIntExtra", "(Ljava/lang/String;I)I", &[JValue::Object(&level_key), JValue::Int(-1)])?.i()?;
                let scale = env.call_method(&battery, "getIntExtra", "(Ljava/lang/String;I)I", &[JValue::Object(&scale_key), JValue::Int(-1)])?.i()?;
                if level >= 0 && scale > 0 { battery_percent = Some((100i64 * level as i64 / scale as i64).clamp(0, 100) as u8); }
            }
            let music_active = false; // Compatibility field; sessions are fetched independently.
            Ok(DeviceSnapshot { battery_percent, music_active, screen_seconds: read_usage(env, activity,last_24h)? })
        })
    }

    fn media(&self, _command: MediaCommand) -> Result<()> {
        anyhow::bail!("対象を指定したメディアセッション操作が必要です")
    }

    fn media_access_settings(&self) -> Result<()> {
        self.on_main(move |env, activity, _| {
            let intent = intent(
                env,
                "android.settings.ACTION_NOTIFICATION_LISTENER_SETTINGS",
            )?;
            start(env, activity, &intent)
        })
    }

    fn media_sessions(&self) -> Result<Vec<MediaSession>> {
        media_bridge::sessions(self)
    }
    fn control_session(&self, id: &str, action: MediaAction) -> Result<()> {
        media_bridge::control(self, id, action)
    }

    fn take_cancelled_widgets(&self) -> Vec<i32> {
        widget_results::take_cancelled()
    }

    fn widget_providers(&self) -> Result<Vec<WidgetProvider>> {
        self.jni(|env, activity| {
            let manager = manager(env, activity)?;
            let pm = env
                .call_method(
                    activity,
                    "getPackageManager",
                    "()Landroid/content/pm/PackageManager;",
                    &[],
                )?
                .l()?;
            let list = env
                .call_method(manager, "getInstalledProviders", "()Ljava/util/List;", &[])?
                .l()?;
            let length = env.call_method(&list, "size", "()I", &[])?.i()?;
            let mut providers = Vec::new();
            for index in 0..length {
                providers.push(env.with_local_frame(16, |env| -> Result<WidgetProvider> {
                    let info = env
                        .call_method(&list, "get", "(I)Ljava/lang/Object;", &[JValue::Int(index)])?
                        .l()?;
                    let provider = env
                        .get_field(&info, "provider", "Landroid/content/ComponentName;")?
                        .l()?;
                    let flat = env
                        .call_method(provider, "flattenToString", "()Ljava/lang/String;", &[])?
                        .l()?;
                    let label = env
                        .call_method(
                            info,
                            "loadLabel",
                            "(Landroid/content/pm/PackageManager;)Ljava/lang/String;",
                            &[JValue::Object(&pm)],
                        )?
                        .l()?;
                    Ok(WidgetProvider {
                        component: java_text(env, &flat)?,
                        label: java_text(env, &label)?,
                    })
                })?);
            }
            providers.sort_by_cached_key(|provider| provider.label.to_lowercase());
            Ok(providers)
        })
    }

    fn add_widget(&self, provider: &str) -> Result<i32> {
        let provider = provider.to_owned();
        self.on_main(move |env, activity, state| {
            let host = ensure_host(env, activity, state)?;
            let id = env
                .call_method(host.as_obj(), "allocateAppWidgetId", "()I", &[])?
                .i()?;
            let result = (|| -> Result<()> {
                let manager = manager(env, activity)?;
                let provider = component(env, &provider)?;
                let bound = env
                    .call_method(
                        manager,
                        "bindAppWidgetIdIfAllowed",
                        "(ILandroid/content/ComponentName;)Z",
                        &[JValue::Int(id), JValue::Object(&provider)],
                    )?
                    .z()?;
                if !bound {
                    let intent = intent(env, "android.appwidget.action.APPWIDGET_BIND")?;
                    extra_id(env, &intent, id)?;
                    let key = env.new_string("appWidgetProvider")?;
                    env.call_method(
                        &intent,
                        "putExtra",
                        "(Ljava/lang/String;Landroid/os/Parcelable;)Landroid/content/Intent;",
                        &[JValue::Object(&key), JValue::Object(&provider)],
                    )?;
                    start_widget_activity(env, activity, &intent)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                let _ = env.exception_clear();
                let _ = env.call_method(
                    host.as_obj(),
                    "deleteAppWidgetId",
                    "(I)V",
                    &[JValue::Int(id)],
                );
                return Err(error);
            }
            state.configure_pending.insert(id);
            Ok(id)
        })
    }

    fn remove_widget(&self, id: i32) -> Result<()> {
        self.on_main(move |env, activity, state| {
            let host = ensure_host(env, activity, state)?;
            env.call_method(
                host.as_obj(),
                "deleteAppWidgetId",
                "(I)V",
                &[JValue::Int(id)],
            )?;
            state.views.remove(&id);
            state.configure_pending.remove(&id);
            state.widgets.retain(|widget| widget.id != id);
            Ok(())
        })
    }

    fn poll_widgets(&self, ids: &[i32], names: &WidgetNames) {
        let ids = ids.to_vec();
        let names = names.clone();
        let this = self.clone();
        (self.main)(Box::new(move || {
            let result = this.jni(|env, activity| {
                if let Some(insets) = read_window_insets(env, activity)? {
                    for (value, inset) in this.system_insets.iter().zip(insets) { value.store(inset, Ordering::Relaxed); }
                }
                apply_system_bars(env, activity, this.dark.load(Ordering::Relaxed))?;
                let mut state = this.state.lock().unwrap();
                poll_prompt(env, &mut state)?;
                if ids.is_empty() && state.host.is_none() { return Ok(()); }
                let host = ensure_host(env, activity, &mut state)?;
                let manager = manager(env, activity)?;
                let pm = env.call_method(activity, "getPackageManager", "()Landroid/content/pm/PackageManager;", &[])?.l()?;
                let mut widgets = Vec::new();
                for id in ids {
                    let mut title = widget_title(id, &names, None);
                    let widget = env.with_local_frame(64, |env| -> Result<TextWidget> {
                        let info = env.call_method(&manager, "getAppWidgetInfo", "(I)Landroid/appwidget/AppWidgetProviderInfo;", &[JValue::Int(id)])?.l()?;
                        if info.is_null() {
                            return Ok(TextWidget { id, title: title.clone(), lines: vec!["許可待ち".into()], pending: true });
                        }
                        // Read the provider's label before inflating/extracting RemoteViews.
                        // A provider's unsupported content must not discard its real name.
                        let label = env.call_method(&info, "loadLabel", "(Landroid/content/pm/PackageManager;)Ljava/lang/String;", &[JValue::Object(&pm)])?.l()?;
                        title = widget_title(id, &names, Some(&java_text(env, &label)?));
                        if state.configure_pending.remove(&id) {
                            let configure = env.get_field(&info, "configure", "Landroid/content/ComponentName;")?.l()?;
                            if !configure.is_null() {
                                let intent = intent(env, "android.appwidget.action.APPWIDGET_CONFIGURE")?;
                                env.call_method(&intent, "setComponent", "(Landroid/content/ComponentName;)Landroid/content/Intent;", &[JValue::Object(&configure)])?;
                                extra_id(env, &intent, id)?;
                                start_widget_activity(env, activity, &intent)?;
                            }
                        }
                        if let std::collections::hash_map::Entry::Vacant(entry) = state.views.entry(id) {
                            let view = env.call_method(host.as_obj(), "createView", "(Landroid/content/Context;ILandroid/appwidget/AppWidgetProviderInfo;)Landroid/appwidget/AppWidgetHostView;", &[JValue::Object(activity), JValue::Int(id), JValue::Object(&info)])?.l()?;
                            // Give RemoteViews a useful size even though egui displays only its text.
                            env.call_method(&view, "updateAppWidgetSize", "(Landroid/os/Bundle;IIII)V", &[JValue::Object(&JObject::null()), JValue::Int(280), JValue::Int(120), JValue::Int(360), JValue::Int(240)])?;
                            // Measure/layout is required by some providers before populating text.
                            env.call_method(&view, "measure", "(II)V", &[JValue::Int(0x40000400), JValue::Int(0x80000400u32 as i32)])?;
                            env.call_method(&view, "layout", "(IIII)V", &[JValue::Int(0), JValue::Int(0), JValue::Int(1024), JValue::Int(1024)])?;
                            entry.insert(env.new_global_ref(view)?);
                        }
                        let mut lines = Vec::new();
                        extract_text(env, state.views[&id].as_obj(), &mut lines, 0, &mut 512)?;
                        lines.retain(|line|line.trim()!=title.trim());
                        if lines.is_empty() { lines.push("内容を表示できません".into()); }
                        Ok(TextWidget { id, title: title.clone(), lines, pending: false })
                    });
                    widgets.push(match widget {
                        Ok(widget) => widget,
                        Err(error) => {
                            // A single incompatible RemoteViews must not hide other widgets.
                            let _ = env.exception_clear();
                            TextWidget { id, title, lines: vec![format!("読み取りに対応できません: {error:#}")], pending: false }
                        }
                    });
                }
                state.widgets = widgets;
                Ok(())
            });
            if let Err(error) = result {
                this.state.lock().unwrap().error = Some(format!("ウィジェット: {error:#}"));
            }
        }));
    }

    fn widgets(&self) -> Vec<TextWidget> {
        self.state.lock().unwrap().widgets.clone()
    }

    fn try_widgets(&self) -> Option<Vec<TextWidget>> {
        self.state
            .try_lock()
            .ok()
            .map(|state| state.widgets.clone())
    }

    fn window_insets(&self) -> [f32; 4] {
        let native = (self.insets)();
        std::array::from_fn(|index| {
            native[index].max(self.system_insets[index].load(Ordering::Relaxed) as f32)
        })
    }

    fn set_dark_theme(&self, dark: bool) {
        self.dark.store(dark, Ordering::Relaxed);
    }

    fn request_text(&self, key: &str, title: &str, initial: &str) -> Result<()> {
        let (key, title, initial) = (key.to_owned(), title.to_owned(), initial.to_owned());
        self.on_main(move |env, activity, state| {
            if state.prompt.is_some() { return Ok(()); }
            // Native EditText supports Japanese IME composition, which NativeActivity's
            // key-event-only input path cannot reliably provide to egui.
            let edit = env.new_object("android/widget/EditText", "(Landroid/content/Context;)V", &[JValue::Object(activity)])?;
            let text = env.new_string(initial)?;
            env.call_method(&edit, "setText", "(Ljava/lang/CharSequence;)V", &[JValue::Object(&text)])?;
            env.call_method(&edit, "setSingleLine", "(Z)V", &[JValue::Bool(1)])?;
            let builder = env.new_object("android/app/AlertDialog$Builder", "(Landroid/content/Context;)V", &[JValue::Object(activity)])?;
            env.call_method(&builder, "setCancelable", "(Z)Landroid/app/AlertDialog$Builder;", &[JValue::Bool(0)])?;
            let title = env.new_string(title)?;
            env.call_method(&builder, "setTitle", "(Ljava/lang/CharSequence;)Landroid/app/AlertDialog$Builder;", &[JValue::Object(&title)])?;
            env.call_method(&builder, "setView", "(Landroid/view/View;)Landroid/app/AlertDialog$Builder;", &[JValue::Object(&edit)])?;
            let done = env.new_string("完了")?;
            env.call_method(&builder, "setPositiveButton", "(Ljava/lang/CharSequence;Landroid/content/DialogInterface$OnClickListener;)Landroid/app/AlertDialog$Builder;", &[JValue::Object(&done), JValue::Object(&JObject::null())])?;
            let dialog = env.call_method(&builder, "show", "()Landroid/app/AlertDialog;", &[])?.l()?;
            env.call_method(&edit, "requestFocus", "()Z", &[])?;
            let window = env.call_method(&dialog, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
            env.call_method(window, "setSoftInputMode", "(I)V", &[JValue::Int(5)])?;
            state.prompt = Some((key, env.new_global_ref(edit)?, env.new_global_ref(dialog)?));
            Ok(())
        })
    }

    fn take_text(&self) -> Option<(String, String)> {
        self.state.try_lock().ok()?.text_result.take()
    }

    fn set_search_field(&self, field: Option<SearchField>) {
        let this = self.clone();
        (self.main)(Box::new(move || {
            let result = this.jni(|env, activity| {
                let mut state = this.state.lock().unwrap();
                let Some(field) = field else {
                    if let Some(edit) = &state.search_edit {
                        hide_keyboard(env, activity, edit.as_obj())?;
                    }
                    if let Some(popup) = &state.search_popup {
                        env.call_method(popup.as_obj(), "dismiss", "()V", &[])?;
                    }
                    state.search_visible = false;
                    state.search_editing = false;
                    state.search_had_ime = false;
                    this.ime_bottom.store(0, Ordering::Relaxed);
                    return Ok(());
                };
                if state.search_edit.is_none() {
                    let edit = env.new_object(
                        "android/widget/EditText",
                        "(Landroid/content/Context;)V",
                        &[JValue::Object(activity)],
                    )?;
                    env.call_method(&edit, "setSingleLine", "(Z)V", &[JValue::Bool(1)])?;
                    env.call_method(&edit, "setTextSize", "(F)V", &[JValue::Float(18.0)])?;
                    env.call_method(&edit, "setBackgroundColor", "(I)V", &[JValue::Int(0)])?;
                    env.call_method(
                        &edit,
                        "setPadding",
                        "(IIII)V",
                        &[
                            JValue::Int(4),
                            JValue::Int(0),
                            JValue::Int(4),
                            JValue::Int(0),
                        ],
                    )?;
                    env.call_method(&edit, "setInputType", "(I)V", &[JValue::Int(1)])?;
                    env.call_method(
                        &edit,
                        "setImeOptions",
                        "(I)V",
                        &[JValue::Int(6 | 0x10000000)],
                    )?;
                    let description = env.new_string("アプリを検索")?;
                    env.call_method(
                        &edit,
                        "setContentDescription",
                        "(Ljava/lang/CharSequence;)V",
                        &[JValue::Object(&description)],
                    )?;
                    // NativeActivity owns its entire window surface. A separate standard
                    // popup surface makes EditText visible and lets Android dispatch IME/key input.
                    let popup = env.new_object(
                        "android/widget/PopupWindow",
                        "(Landroid/view/View;IIZ)V",
                        &[
                            JValue::Object(&edit),
                            JValue::Int(field.rect[2]),
                            JValue::Int(field.rect[3]),
                            JValue::Bool(1),
                        ],
                    )?;
                    env.call_method(
                        &popup,
                        "setBackgroundDrawable",
                        "(Landroid/graphics/drawable/Drawable;)V",
                        &[JValue::Object(&JObject::null())],
                    )?;
                    env.call_method(&popup, "setOutsideTouchable", "(Z)V", &[JValue::Bool(0)])?;
                    env.call_method(&popup, "setInputMethodMode", "(I)V", &[JValue::Int(1)])?;
                    env.call_method(&popup, "setSoftInputMode", "(I)V", &[JValue::Int(0x30)])?;
                    env.call_method(&popup, "setClippingEnabled", "(Z)V", &[JValue::Bool(0)])?;
                    if sdk_version(env)? >= 29 {
                        env.call_method(&popup, "setTouchModal", "(Z)V", &[JValue::Bool(0)])?;
                        env.call_method(
                            &popup,
                            "setIsLaidOutInScreen",
                            "(Z)V",
                            &[JValue::Bool(1)],
                        )?;
                    }
                    state.search_edit = Some(env.new_global_ref(edit)?);
                    state.search_popup = Some(env.new_global_ref(popup)?);
                }
                let edit = state.search_edit.as_ref().unwrap().as_obj();
                let popup = state.search_popup.as_ref().unwrap().as_obj();
                env.call_method(
                    edit,
                    "setTextSize",
                    "(F)V",
                    &[JValue::Float(18.0 * field.font_percent as f32 / 100.0)],
                )?;
                let color = if field.dark {
                    0xffe7e6dbu32 as i32
                } else {
                    0xff22241fu32 as i32
                };
                let background = if field.dark {
                    0xff181a17u32 as i32
                } else {
                    0xfff4f1e9u32 as i32
                };
                env.call_method(
                    edit,
                    "setBackgroundColor",
                    "(I)V",
                    &[JValue::Int(background)],
                )?;
                env.call_method(edit, "setTextColor", "(I)V", &[JValue::Int(color)])?;
                if sdk_version(env)? >= 29 {
                    let cursor =
                        env.new_object("android/graphics/drawable/GradientDrawable", "()V", &[])?;
                    env.call_method(&cursor, "setColor", "(I)V", &[JValue::Int(color)])?;
                    env.call_method(
                        &cursor,
                        "setSize",
                        "(II)V",
                        &[JValue::Int(3), JValue::Int(field.rect[3])],
                    )?;
                    env.call_method(
                        edit,
                        "setTextCursorDrawable",
                        "(Landroid/graphics/drawable/Drawable;)V",
                        &[JValue::Object(&cursor)],
                    )?;
                }
                if !state.search_visible {
                    let text = env.new_string(&field.text)?;
                    env.call_method(
                        edit,
                        "setText",
                        "(Ljava/lang/CharSequence;)V",
                        &[JValue::Object(&text)],
                    )?;
                }
                let editing = state.search_editing || field.focus;
                env.call_method(
                    popup,
                    "setFocusable",
                    "(Z)V",
                    &[JValue::Bool(editing as u8)],
                )?;
                env.call_method(
                    popup,
                    "setTouchable",
                    "(Z)V",
                    &[JValue::Bool(editing as u8)],
                )?;
                if env.call_method(popup, "isShowing", "()Z", &[])?.z()? {
                    env.call_method(popup, "update", "(IIII)V", &field.rect.map(JValue::Int))?;
                } else {
                    env.call_method(popup, "setWidth", "(I)V", &[JValue::Int(field.rect[2])])?;
                    env.call_method(popup, "setHeight", "(I)V", &[JValue::Int(field.rect[3])])?;
                    let window = env
                        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
                        .l()?;
                    let decor = env
                        .call_method(window, "getDecorView", "()Landroid/view/View;", &[])?
                        .l()?;
                    env.call_method(
                        popup,
                        "showAtLocation",
                        "(Landroid/view/View;III)V",
                        &[
                            JValue::Object(&decor),
                            JValue::Int(0x33),
                            JValue::Int(field.rect[0]),
                            JValue::Int(field.rect[1]),
                        ],
                    )?;
                }
                if field.focus {
                    env.call_method(edit, "requestFocus", "()Z", &[])?;
                    let imm = service(env, activity, "input_method")?;
                    env.call_method(
                        imm,
                        "showSoftInput",
                        "(Landroid/view/View;I)Z",
                        &[JValue::Object(edit), JValue::Int(1)],
                    )?;
                }
                state.search_editing = editing;
                if field.focus {
                    state.search_focus_attempts = 12;
                    state.search_retry_at = Some(Instant::now());
                }
                state.search_visible = true;
                Ok(())
            });
            if let Err(error) = result {
                this.state.lock().unwrap().error = Some(format!("検索欄: {error:#}"));
            }
        }));
    }

    fn poll_search_field(&self) {
        let this = self.clone();
        (self.main)(Box::new(move || {
            let result = this.jni(|env, activity| {
                let mut state = this.state.lock().unwrap();
                if !state.search_visible {
                    return Ok(());
                }
                if sdk_version(env)? >= 30 {
                    let window = env
                        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
                        .l()?;
                    let decor = env
                        .call_method(window, "getDecorView", "()Landroid/view/View;", &[])?
                        .l()?;
                    let insets = env
                        .call_method(
                            decor,
                            "getRootWindowInsets",
                            "()Landroid/view/WindowInsets;",
                            &[],
                        )?
                        .l()?;
                    if !insets.is_null() {
                        let ime = env
                            .call_static_method(
                                "android/view/WindowInsets$Type",
                                "ime",
                                "()I",
                                &[],
                            )?
                            .i()?;
                        let inset = env
                            .call_method(
                                insets,
                                "getInsets",
                                "(I)Landroid/graphics/Insets;",
                                &[JValue::Int(ime)],
                            )?
                            .l()?;
                        this.ime_bottom
                            .store(env.get_field(inset, "bottom", "I")?.i()?, Ordering::Relaxed);
                    }
                } else {
                    // Before WindowInsets.Type.ime(), measure the visible window frame.
                    let window = env
                        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
                        .l()?;
                    let decor = env
                        .call_method(window, "getDecorView", "()Landroid/view/View;", &[])?
                        .l()?;
                    let frame = env.new_object("android/graphics/Rect", "()V", &[])?;
                    env.call_method(
                        &decor,
                        "getWindowVisibleDisplayFrame",
                        "(Landroid/graphics/Rect;)V",
                        &[JValue::Object(&frame)],
                    )?;
                    let height = env.call_method(decor, "getHeight", "()I", &[])?.i()?;
                    let bottom = env.get_field(frame, "bottom", "I")?.i()?;
                    this.ime_bottom.store(
                        if height - bottom > height / 5 {
                            height - bottom
                        } else {
                            0
                        },
                        Ordering::Relaxed,
                    );
                }
                let popup = state.search_popup.as_ref().unwrap().as_obj();
                if !env.call_method(popup, "isShowing", "()Z", &[])?.z()? {
                    let key = if state.search_had_ime {
                        "search_blurred"
                    } else {
                        "home"
                    };
                    state.search_editing = false;
                    state.search_had_ime = false;
                    state.text_result = Some((key.into(), state.search_text.clone()));
                    return Ok(());
                }
                let ime_visible = this.ime_bottom.load(Ordering::Relaxed) > 0;
                if state.search_editing
                    && !ime_visible
                    && state.search_focus_attempts > 0
                    && state
                        .search_retry_at
                        .is_none_or(|at| at.elapsed() >= Duration::from_millis(80))
                {
                    state.search_focus_attempts -= 1;
                    state.search_retry_at = Some(Instant::now());
                    let imm = service(env, activity, "input_method")?;
                    env.call_method(
                        imm,
                        "showSoftInput",
                        "(Landroid/view/View;I)Z",
                        &[
                            JValue::Object(state.search_edit.as_ref().unwrap().as_obj()),
                            JValue::Int(1),
                        ],
                    )?;
                }
                if state.search_had_ime && !ime_visible {
                    blur_search(env, &state)?;
                    state.search_editing = false;
                    state.search_had_ime = false;
                    state.text_result = Some(("home".into(), String::new()));
                    return Ok(());
                } else if ime_visible {
                    state.search_had_ime = true;
                    state.search_focus_attempts = 0;
                }
                if let Some(edit) = &state.search_edit {
                    let text = env
                        .call_method(edit.as_obj(), "getText", "()Landroid/text/Editable;", &[])?
                        .l()?;
                    let text = java_text(env, &text)?;
                    if text != state.search_text {
                        state.search_text = text.clone();
                        state.text_result = Some(("search".into(), text));
                    }
                }
                Ok(())
            });
            if let Err(error) = result {
                this.state.lock().unwrap().error = Some(format!("検索: {error:#}"));
            }
        }));
    }

    fn dismiss_keyboard(&self) {
        let this = self.clone();
        (self.main)(Box::new(move || {
            let _ = this.jni(|env, activity| {
                let mut state = this.state.lock().unwrap();
                if state.search_editing {
                    if let Some(edit) = &state.search_edit {
                        hide_keyboard(env, activity, edit.as_obj())?;
                    }
                    blur_search(env, &state)?;
                    state.search_editing = false;
                    state.search_had_ime = false;
                    state.search_focus_attempts = 0;
                }
                Ok(())
            });
        }));
    }
    fn keyboard_inset(&self) -> f32 {
        self.ime_bottom.load(Ordering::Relaxed) as f32
    }

    fn take_error(&self) -> Option<String> {
        self.state.try_lock().ok()?.error.take()
    }
}

fn blur_search(env: &mut JNIEnv<'_>, state: &JavaState) -> Result<()> {
    if let Some(popup) = &state.search_popup {
        env.call_method(popup.as_obj(), "setFocusable", "(Z)V", &[JValue::Bool(0)])?;
        env.call_method(popup.as_obj(), "setTouchable", "(Z)V", &[JValue::Bool(0)])?;
        env.call_method(popup.as_obj(), "update", "()V", &[])?;
    }
    Ok(())
}

fn hide_keyboard(env: &mut JNIEnv<'_>, activity: &JObject<'_>, edit: &JObject<'_>) -> Result<()> {
    let token = env
        .call_method(edit, "getWindowToken", "()Landroid/os/IBinder;", &[])?
        .l()?;
    let imm = service(env, activity, "input_method")?;
    env.call_method(
        imm,
        "hideSoftInputFromWindow",
        "(Landroid/os/IBinder;I)Z",
        &[JValue::Object(&token), JValue::Int(0)],
    )?;
    env.call_method(edit, "clearFocus", "()V", &[])?;
    Ok(())
}

fn poll_prompt(env: &mut JNIEnv<'_>, state: &mut JavaState) -> Result<()> {
    if let Some((_, _, dialog)) = &state.prompt {
        if !env
            .call_method(dialog.as_obj(), "isShowing", "()Z", &[])?
            .z()?
        {
            let (key, edit, _) = state.prompt.take().unwrap();
            let text = env
                .call_method(edit.as_obj(), "getText", "()Landroid/text/Editable;", &[])?
                .l()?;
            state.text_result = Some((key, java_text(env, &text)?));
        }
    }
    Ok(())
}

fn extract_text(
    env: &mut JNIEnv<'_>,
    view: &JObject<'_>,
    lines: &mut Vec<String>,
    depth: usize,
    budget: &mut usize,
) -> Result<()> {
    if depth > 32 || *budget == 0 || lines.len() >= 64 {
        return Ok(());
    }
    *budget -= 1;
    if env.call_method(view, "getVisibility", "()I", &[])?.i()? != 0 {
        return Ok(());
    }
    let mut text = String::new();
    if env.is_instance_of(view, "android/widget/TextView")? {
        let value = env
            .call_method(view, "getText", "()Ljava/lang/CharSequence;", &[])?
            .l()?;
        text = java_text(env, &value)?.trim().to_owned();
    }
    if text.is_empty() {
        let value = env
            .call_method(
                view,
                "getContentDescription",
                "()Ljava/lang/CharSequence;",
                &[],
            )?
            .l()?;
        text = java_text(env, &value)?.trim().to_owned();
    }
    let remaining = 4096usize.saturating_sub(lines.iter().map(|s| s.chars().count()).sum());
    let text: String = text.chars().take(remaining).collect();
    if !text.is_empty() && !lines.contains(&text) {
        lines.push(text);
    }
    if env.is_instance_of(view, "android/view/ViewGroup")? {
        let length = env.call_method(view, "getChildCount", "()I", &[])?.i()?;
        for index in 0..length {
            if *budget == 0 || lines.len() >= 64 {
                break;
            }
            env.with_local_frame(16, |env| -> Result<()> {
                let child = env
                    .call_method(
                        view,
                        "getChildAt",
                        "(I)Landroid/view/View;",
                        &[JValue::Int(index)],
                    )?
                    .l()?;
                extract_text(env, &child, lines, depth + 1, budget)
            })?;
        }
    }
    Ok(())
}

fn sdk_version(env: &mut JNIEnv<'_>) -> Result<i32> {
    Ok(env
        .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
        .i()?)
}

fn read_window_insets(env: &mut JNIEnv<'_>, activity: &JObject<'_>) -> Result<Option<[i32; 4]>> {
    let window = env
        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
        .l()?;
    let decor = env
        .call_method(window, "getDecorView", "()Landroid/view/View;", &[])?
        .l()?;
    let insets = env
        .call_method(
            decor,
            "getRootWindowInsets",
            "()Landroid/view/WindowInsets;",
            &[],
        )?
        .l()?;
    if insets.is_null() {
        return Ok(None);
    }
    let mut result = [0; 4];
    if sdk_version(env)? >= 30 {
        let bars = env
            .call_static_method("android/view/WindowInsets$Type", "systemBars", "()I", &[])?
            .i()?;
        let cutout = env
            .call_static_method(
                "android/view/WindowInsets$Type",
                "displayCutout",
                "()I",
                &[],
            )?
            .i()?;
        let safe = env
            .call_method(
                insets,
                "getInsetsIgnoringVisibility",
                "(I)Landroid/graphics/Insets;",
                &[JValue::Int(bars | cutout)],
            )?
            .l()?;
        for (value, field) in result.iter_mut().zip(["left", "top", "right", "bottom"]) {
            *value = env.get_field(&safe, field, "I")?.i()?;
        }
    } else {
        for (value, getter) in result.iter_mut().zip([
            "getStableInsetLeft",
            "getStableInsetTop",
            "getStableInsetRight",
            "getStableInsetBottom",
        ]) {
            *value = env.call_method(&insets, getter, "()I", &[])?.i()?;
        }
        let cutout = env
            .call_method(
                &insets,
                "getDisplayCutout",
                "()Landroid/view/DisplayCutout;",
                &[],
            )?
            .l()?;
        if !cutout.is_null() {
            for (value, getter) in result.iter_mut().zip([
                "getSafeInsetLeft",
                "getSafeInsetTop",
                "getSafeInsetRight",
                "getSafeInsetBottom",
            ]) {
                *value = (*value).max(env.call_method(&cutout, getter, "()I", &[])?.i()?);
            }
        }
    }
    Ok(Some(result))
}

fn apply_system_bars(env: &mut JNIEnv<'_>, activity: &JObject<'_>, dark: bool) -> Result<()> {
    let window = env
        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
        .l()?;
    if sdk_version(env)? >= 30 {
        let controller = env
            .call_method(
                window,
                "getInsetsController",
                "()Landroid/view/WindowInsetsController;",
                &[],
            )?
            .l()?;
        if !controller.is_null() {
            // APPEARANCE_LIGHT_STATUS_BARS | APPEARANCE_LIGHT_NAVIGATION_BARS.
            env.call_method(
                controller,
                "setSystemBarsAppearance",
                "(II)V",
                &[JValue::Int(if dark { 0 } else { 24 }), JValue::Int(24)],
            )?;
        }
    } else {
        let decor = env
            .call_method(window, "getDecorView", "()Landroid/view/View;", &[])?
            .l()?;
        let flags = env
            .call_method(&decor, "getSystemUiVisibility", "()I", &[])?
            .i()?;
        let flags = if dark {
            flags & !0x2010
        } else {
            flags | 0x2010
        };
        env.call_method(
            decor,
            "setSystemUiVisibility",
            "(I)V",
            &[JValue::Int(flags)],
        )?;
    }
    Ok(())
}

fn read_usage(env: &mut JNIEnv<'_>, activity: &JObject<'_>, last_24h: bool) -> Result<Option<u64>> {
    let ops = service(env, activity, "appops")?;
    let op = env.new_string("android:get_usage_stats")?;
    let uid = env
        .call_static_method("android/os/Process", "myUid", "()I", &[])?
        .i()?;
    let package = env
        .call_method(activity, "getPackageName", "()Ljava/lang/String;", &[])?
        .l()?;
    let allowed = env
        .call_method(
            ops,
            "checkOpNoThrow",
            "(Ljava/lang/String;ILjava/lang/String;)I",
            &[
                JValue::Object(&op),
                JValue::Int(uid),
                JValue::Object(&package),
            ],
        )?
        .i()?
        == 0;
    if !allowed {
        return Ok(None);
    }
    let now = Local::now();
    let naive = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let midnight = Local
        .from_local_datetime(&naive)
        .earliest()
        .unwrap_or(now)
        .timestamp_millis();
    let now = now.timestamp_millis();
    let midnight = if last_24h { now - 86_400_000 } else { midnight };
    let manager = service(env, activity, "usagestats")?;
    let events = env
        .call_method(
            manager,
            "queryEvents",
            "(JJ)Landroid/app/usage/UsageEvents;",
            &[JValue::Long(midnight - 7 * 86_400_000), JValue::Long(now)],
        )?
        .l()?;
    let event = env.new_object("android/app/usage/UsageEvents$Event", "()V", &[])?;
    let mut screen = Vec::new();
    while env.call_method(&events, "hasNextEvent", "()Z", &[])?.z()? {
        if !env
            .call_method(
                &events,
                "getNextEvent",
                "(Landroid/app/usage/UsageEvents$Event;)Z",
                &[JValue::Object(&event)],
            )?
            .z()?
        {
            break;
        }
        let kind = env.call_method(&event, "getEventType", "()I", &[])?.i()?;
        if [15, 16, 26, 27].contains(&kind) {
            let at = env.call_method(&event, "getTimeStamp", "()J", &[])?.j()?;
            screen.push(match kind {
                15 => ScreenEvent::On(at),
                16 => ScreenEvent::Off(at),
                _ => ScreenEvent::Reset(at),
            });
        }
    }
    Ok(Some(usage::screen_seconds(&screen, midnight, now)))
}
