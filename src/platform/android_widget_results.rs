//! Result bridge for system widget permission/configuration activities.
use anyhow::Result;
use jni::{
    objects::{JObject, JValue},
    JNIEnv,
};
use std::sync::Mutex;
static CANCELLED: Mutex<Vec<i32>> = Mutex::new(Vec::new());
pub(super) fn take_cancelled() -> Vec<i32> {
    std::mem::take(&mut *CANCELLED.lock().unwrap())
}
fn widget_id(env: &mut JNIEnv<'_>, activity: &JObject<'_>) -> Result<i32> {
    let intent = env
        .call_method(activity, "getIntent", "()Landroid/content/Intent;", &[])?
        .l()?;
    let key = env.new_string("appWidgetId")?;
    Ok(env
        .call_method(
            intent,
            "getIntExtra",
            "(Ljava/lang/String;I)I",
            &[JValue::Object(&key), JValue::Int(-1)],
        )?
        .i()?)
}
fn finish(env: &mut JNIEnv<'_>, activity: &JObject<'_>) {
    let _ = env.call_method(activity, "finish", "()V", &[]);
}
#[no_mangle]
pub extern "system" fn Java_dev_quiet_launcher_WidgetRequestActivity_launchRequest(
    mut env: JNIEnv<'_>,
    activity: JObject<'_>,
) {
    let result = (|| -> Result<()> {
        let intent = env
            .call_method(&activity, "getIntent", "()Landroid/content/Intent;", &[])?
            .l()?;
        let key = env.new_string("quiet.widget.request")?;
        let request = env
            .call_method(
                intent,
                "getParcelableExtra",
                "(Ljava/lang/String;)Landroid/os/Parcelable;",
                &[JValue::Object(&key)],
            )?
            .l()?;
        anyhow::ensure!(!request.is_null(), "widget request missing");
        env.call_method(
            &activity,
            "startActivityForResult",
            "(Landroid/content/Intent;I)V",
            &[JValue::Object(&request), JValue::Int(0x5151)],
        )?;
        Ok(())
    })();
    if result.is_err() {
        let _ = env.exception_clear();
        if let Ok(id) = widget_id(&mut env, &activity) {
            if id >= 0 {
                CANCELLED.lock().unwrap().push(id);
            }
        }
        finish(&mut env, &activity);
    }
}
#[no_mangle]
pub extern "system" fn Java_dev_quiet_launcher_WidgetRequestActivity_onActivityResult(
    mut env: JNIEnv<'_>,
    activity: JObject<'_>,
    _request: i32,
    result: i32,
    _data: JObject<'_>,
) {
    if result != -1 {
        if let Ok(id) = widget_id(&mut env, &activity) {
            if id >= 0 {
                CANCELLED.lock().unwrap().push(id);
            }
        }
    }
    finish(&mut env, &activity);
}
