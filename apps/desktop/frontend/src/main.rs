#![recursion_limit = "512"]

use leptos::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen(module = "/src/tauri.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct ProfileInput {
    id: String,
    tin: String,
    branch_code: String,
    taxpayer_name: String,
    rdo_code: String,
    registered_address: String,
    zip_code: String,
    email_address: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Form1601CInput {
    profile_id: String,
    tin: String,
    email: String,
    month: String,
    year: String,
    amended: bool,
    amendment_number: String,
    tax_withheld_agent: bool,
    sheets: String,
    atc: String,
    rdo_code: String,
    taxpayer_name: String,
    registered_address: String,
    zip_code: String,
    telephone: String,
    category_private: bool,
    special_tax_rate: bool,
    treaty_code: String,
    tax_14: String,
    tax_15: String,
    tax_16: String,
    tax_17: String,
    tax_18: String,
    tax_19: String,
    tax_20_other: String,
    tax_20: String,
    tax_21: String,
    tax_22: String,
    tax_23: String,
    tax_24: String,
    tax_25: String,
    tax_26: String,
    tax_27: String,
    tax_28: String,
    tax_29_other: String,
    tax_29: String,
    tax_30: String,
    tax_31: String,
    tax_32: String,
    tax_33: String,
    tax_34: String,
    tax_35: String,
    tax_36: String,
    payment_agency_37: String,
    payment_number_37: String,
    payment_date_37: String,
    payment_amount_37: String,
    schedule_total_1: String,
    line_of_business: String,
}

impl Default for Form1601CInput {
    fn default() -> Self {
        Self {
            profile_id: "synthetic-redacted-test-profile".into(),
            tin: "123-456-789-00000".into(),
            email: "authorized@example.test".into(),
            month: "06".into(),
            year: "2026".into(),
            amended: true,
            amendment_number: "2".into(),
            tax_withheld_agent: false,
            sheets: "0".into(),
            atc: "WW010".into(),
            rdo_code: "044".into(),
            taxpayer_name: "AUTHORIZED TEST TAXPAYER".into(),
            registered_address: "REDACTED TEST ADDRESS".into(),
            zip_code: "0000".into(),
            telephone: "0000000".into(),
            category_private: true,
            special_tax_rate: false,
            treaty_code: "0".into(),
            tax_14: "0.00".into(),
            tax_15: "0.00".into(),
            tax_16: "0.00".into(),
            tax_17: "0.00".into(),
            tax_18: "0.00".into(),
            tax_19: "0.00".into(),
            tax_20_other: String::new(),
            tax_20: "0.00".into(),
            tax_21: "0.00".into(),
            tax_22: "0.00".into(),
            tax_23: "0.00".into(),
            tax_24: "0.00".into(),
            tax_25: "0.00".into(),
            tax_26: "0.00".into(),
            tax_27: "0.00".into(),
            tax_28: "0.00".into(),
            tax_29_other: String::new(),
            tax_29: "0.00".into(),
            tax_30: "0.00".into(),
            tax_31: "0.00".into(),
            tax_32: "0.00".into(),
            tax_33: "0.00".into(),
            tax_34: "0.00".into(),
            tax_35: "0.00".into(),
            tax_36: "0.00".into(),
            payment_agency_37: String::new(),
            payment_number_37: String::new(),
            payment_date_37: String::new(),
            payment_amount_37: String::new(),
            schedule_total_1: "0.00".into(),
            line_of_business: "BUSINESS%2520SUPPORT%2520SERVICE%2520ACTIVITIES%252C%2520N.E.C.".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SubmissionManifestResponse {
    form_code: String,
    form_version: String,
    remote_directory: String,
    remote_path: String,
    filename: String,
    plaintext_sha256: String,
    payload_sha256: String,
    payload_size: usize,
    #[serde(rename = "period_mmYYYY")]
    period_mm_yyyy: String,
    profile_id: String,
    generated_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PackagePreviewResponse {
    manifest: SubmissionManifestResponse,
    payload_path: String,
    payload_sha256_short: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct JobResponse {
    id: i64,
    form_code: String,
    input_json: String,
    mode: String,
    status: String,
    attempts: u32,
    max_attempts: u32,
    next_attempt_unix_seconds: u64,
    created_unix_seconds: u64,
    updated_unix_seconds: u64,
    submission_idempotency_key: Option<String>,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SafeSubmissionRecordResponse {
    idempotency_key: String,
    idempotency_key_short: String,
    status: String,
    dry_run: bool,
    form_code: String,
    #[serde(rename = "period_mmYYYY")]
    period_mm_yyyy: String,
    profile_id: String,
    remote_path: String,
    filename: String,
    payload_sha256: String,
    payload_sha256_short: String,
    payload_size: usize,
    created_unix_seconds: u64,
    updated_unix_seconds: u64,
    attempts: u32,
    last_error: Option<String>,
    receipt_status: Option<String>,
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (route, set_route) = create_signal("Dashboard".to_string());
    let (status, set_status) = create_signal("Ready. Default mode is dry-run.".to_string());
    let (locked, set_locked) = create_signal(false);
    let (unlock_pin, set_unlock_pin) = create_signal(String::new());
    let (theme, set_theme) = create_signal("system".to_string());
    let (profiles, set_profiles) = create_signal("[]".to_string());
    let (jobs, set_jobs) = create_signal("[]".to_string());
    let (submissions, set_submissions) = create_signal("[]".to_string());
    let (plaintext_preview, set_plaintext_preview) = create_signal("No plaintext preview rendered yet.".to_string());
    let (package_preview, set_package_preview) = create_signal(None::<PackagePreviewResponse>);
    let (latest_package_filename, set_latest_package_filename) = create_signal(None::<String>);
    let (receipt_text, set_receipt_text) = create_signal(sample_receipt_text());
    let (settings_pin, set_settings_pin) = create_signal(String::new());
    let (profile, set_profile) = create_signal(ProfileInput {
        id: "sample-profile".into(),
        tin: "123456789".into(),
        branch_code: "0000".into(),
        taxpayer_name: "Synthetic Taxpayer".into(),
        rdo_code: "081".into(),
        registered_address: "Synthetic Address".into(),
        zip_code: "1000".into(),
        email_address: "synthetic@example.test".into(),
    });
    let (form_1601c, set_form_1601c) = create_signal(Form1601CInput::default());
    let nav = vec![
        "Dashboard",
        "Profiles",
        "1601C",
        "Package",
        "Jobs",
        "Submissions",
        "Receipt",
        "Logs",
        "Settings",
    ];

    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&json!({})).unwrap_or(JsValue::NULL);
        match invoke("app_snapshot", args).await {
            Ok(value) => {
                let snapshot: serde_json::Value =
                    serde_wasm_bindgen::from_value(value).unwrap_or_else(|_| json!({}));
                if let Some(saved_theme) = snapshot
                    .get("settings")
                    .and_then(|settings| settings.get("theme"))
                    .and_then(|theme| theme.as_str())
                    .and_then(normalize_theme)
                {
                    set_theme.set(saved_theme.to_string());
                }
            }
            Err(err) => {
                let msg = err.as_string().unwrap_or_else(|| format!("{err:?}"));
                set_status.set(format!("app_snapshot failed: {msg}"));
            }
        }
    });

    let run_command = move |command: &'static str, args: serde_json::Value, target: &'static str| {
        set_status.set(format!("Running {command}…"));
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&args).unwrap_or(JsValue::NULL);
            match invoke(command, args).await {
                Ok(value) => {
                    let text = js_sys::JSON::stringify(&value)
                        .ok()
                        .and_then(|s| s.as_string())
                        .unwrap_or_else(|| "ok".into());
                    match target {
                        "profiles" => set_profiles.set(text.clone()),
                        "jobs" => set_jobs.set(text.clone()),
                        "submissions" => set_submissions.set(text.clone()),
                        "package" if command == "render_1601c" => {
                            let plaintext = serde_wasm_bindgen::from_value::<String>(value.clone())
                                .unwrap_or_else(|_| text.clone());
                            set_plaintext_preview.set(plaintext);
                        }
                        "package" if command == "package_1601c" => {
                            let parsed = serde_wasm_bindgen::from_value::<PackagePreviewResponse>(value)
                                .ok();
                            if let Some(package) = parsed.as_ref() {
                                set_latest_package_filename.set(Some(package.manifest.filename.clone()));
                            }
                            set_package_preview.set(parsed);
                        }
                        "package" => {} ,
                        _ => {}
                    }
                    if target == "submissions" {
                        if let Some(filename) = latest_submission_filename(&text) {
                            set_latest_package_filename.set(Some(filename));
                        }
                    }
                    if command == "run_queue_dry_run" {
                        let refresh_args = serde_wasm_bindgen::to_value(&json!({})).unwrap_or(JsValue::NULL);
                        if let Ok(submission_value) = invoke("list_submissions", refresh_args).await {
                            let submission_text = js_sys::JSON::stringify(&submission_value)
                                .ok()
                                .and_then(|s| s.as_string())
                                .unwrap_or_else(|| "[]".into());
                            if let Some(filename) = latest_submission_filename(&submission_text) {
                                set_latest_package_filename.set(Some(filename));
                            }
                            set_submissions.set(submission_text);
                        }
                    }
                    set_status.set(format!("{command} completed"));
                }
                Err(err) => {
                    let msg = err.as_string().unwrap_or_else(|| format!("{err:?}"));
                    set_status.set(format!("{command} failed: {msg}"));
                }
            }
        });
    };

    let set_theme_preference = move |theme_name: &'static str| {
        let Some(next_theme) = normalize_theme(theme_name).map(str::to_string) else {
            set_status.set(format!("Invalid theme preference: {theme_name}"));
            return;
        };
        let previous_theme = theme.get_untracked();
        set_theme.set(next_theme.clone());
        set_status.set(format!("Saving {next_theme} theme preference…"));
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&json!({"theme": next_theme})).unwrap_or(JsValue::NULL);
            match invoke("update_settings", args).await {
                Ok(value) => {
                    let settings: serde_json::Value =
                        serde_wasm_bindgen::from_value(value).unwrap_or_else(|_| json!({}));
                    let saved_theme = settings
                        .get("theme")
                        .and_then(|theme| theme.as_str())
                        .and_then(normalize_theme)
                        .unwrap_or(theme_name);
                    set_theme.set(saved_theme.to_string());
                    set_status.set(format!("Theme preference saved: {saved_theme}"));
                }
                Err(err) => {
                    let msg = err.as_string().unwrap_or_else(|| format!("{err:?}"));
                    set_theme.set(previous_theme);
                    set_status.set(format!("update_settings failed; theme preference reverted: {msg}"));
                }
            }
        });
    };

    let lock_now = move || {
        let pin_value = settings_pin.get_untracked();
        if pin_value.len() != 4 || !pin_value.chars().all(|ch| ch.is_ascii_digit()) {
            set_status.set("Enter exactly four digits before locking.".to_string());
            return;
        }
        set_status.set("Saving PIN and locking app…".to_string());
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&json!({"pin": pin_value})).unwrap_or(JsValue::NULL);
            match invoke("lock_init", args).await {
                Ok(_) => {
                    set_unlock_pin.set(String::new());
                    set_locked.set(true);
                    set_status.set("App locked. Enter your 4-digit PIN to unlock.".to_string());
                }
                Err(err) => {
                    let msg = err.as_string().unwrap_or_else(|| format!("{err:?}"));
                    set_status.set(format!("lock_init failed: {msg}"));
                }
            }
        });
    };

    let unlock_app = move || {
        let pin_value = unlock_pin.get_untracked();
        if pin_value.len() != 4 || !pin_value.chars().all(|ch| ch.is_ascii_digit()) {
            set_status.set("Enter the 4-digit PIN to unlock.".to_string());
            return;
        }
        set_status.set("Checking PIN…".to_string());
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&json!({"pin": pin_value})).unwrap_or(JsValue::NULL);
            match invoke("unlock_check", args).await {
                Ok(value) => {
                    let ok = serde_wasm_bindgen::from_value::<bool>(value).unwrap_or(false);
                    if ok {
                        set_unlock_pin.set(String::new());
                        set_locked.set(false);
                        set_status.set("Unlocked.".to_string());
                    } else {
                        set_status.set("Incorrect PIN.".to_string());
                    }
                }
                Err(err) => {
                    let msg = err.as_string().unwrap_or_else(|| format!("{err:?}"));
                    set_status.set(format!("unlock_check failed: {msg}"));
                }
            }
        });
    };

    view! {
        <main class=move || format!("app theme-{}", theme.get())>
            <aside class="sidebar">
                <h1>"eBIRForms"</h1>
                <p class="muted">"Desktop MVP · dry-run first"</p>
                <nav>
                    {nav.into_iter().map(|item| {
                        let label = item.to_string();
                        view! { <button on:click=move |_| set_route.set(label.clone())>{item}</button> }
                    }).collect_view()}
                </nav>
            </aside>
            <section class="content">
                <div class="status">{move || status.get()}</div>
                {move || if locked.get() {
                    view! { <LockScreen pin=unlock_pin set_pin=set_unlock_pin unlock_app=unlock_app /> }.into_view()
                } else { match route.get().as_str() {
                    "Profiles" => view! { <Profiles profile=profile set_profile=set_profile profiles=profiles run_command=run_command /> }.into_view(),
                    "1601C" => view! { <Form1601C form=form_1601c set_form=set_form_1601c run_command=run_command /> }.into_view(),
                    "Package" => view! { <PackagePreview plaintext_preview=plaintext_preview package_preview=package_preview /> }.into_view(),
                    "Jobs" => view! { <Jobs jobs=jobs run_command=run_command /> }.into_view(),
                    "Submissions" => view! { <Submissions submissions=submissions run_command=run_command /> }.into_view(),
                    "Receipt" => view! { <Receipt receipt_text=receipt_text set_receipt_text=set_receipt_text latest_package_filename=latest_package_filename submissions=submissions run_command=run_command /> }.into_view(),
                    "Logs" => view! { <Logs /> }.into_view(),
                    "Settings" => view! { <Settings theme=theme set_theme_preference=set_theme_preference pin=settings_pin set_pin=set_settings_pin lock_now=lock_now /> }.into_view(),
                    _ => view! { <Dashboard profiles=profiles jobs=jobs submissions=submissions run_command=run_command /> }.into_view(),
                }}}
            </section>
        </main>
    }
}

#[component]
fn Dashboard<F>(profiles: ReadSignal<String>, jobs: ReadSignal<String>, submissions: ReadSignal<String>, run_command: F) -> impl IntoView
where
    F: Fn(&'static str, serde_json::Value, &'static str) + Copy + 'static,
{
    view! {
        <section class="card-grid">
            <Panel title="Dashboard">
                <p>"Use the left navigation to create a profile, package a synthetic 1601C, queue dry-run jobs, and inspect safe submission metadata."</p>
                <button on:click=move |_| run_command("list_profiles", json!({}), "profiles")>"Refresh profiles"</button>
                <button on:click=move |_| run_command("list_jobs", json!({}), "jobs")>"Refresh jobs"</button>
                <button on:click=move |_| run_command("list_submissions", json!({}), "submissions")>"Refresh submissions"</button>
            </Panel>
            <Panel title="Safe summaries">
                <h3>"Profiles"</h3><pre>{move || profiles.get()}</pre>
                <h3>"Jobs"</h3><pre>{move || jobs.get()}</pre>
                <h3>"Submissions"</h3><pre>{move || submissions.get()}</pre>
            </Panel>
        </section>
    }
}

#[component]
fn LockScreen<F>(pin: ReadSignal<String>, set_pin: WriteSignal<String>, unlock_app: F) -> impl IntoView
where
    F: Fn() + Copy + 'static,
{
    view! {
        <section class="lock-screen">
            <div class="lock-card">
                <div class="lock-icon">"🔒"</div>
                <h2>"App locked"</h2>
                <p class="muted">"Enter your 4-digit PIN to unlock the desktop UI."</p>
                <input
                    class="pin-input"
                    type="password"
                    inputmode="numeric"
                    maxlength="4"
                    placeholder="••••"
                    prop:value=pin
                    on:input=move |ev| set_pin.set(four_digit_pin(event_target_value(&ev)))
                />
                <button on:click=move |_| unlock_app()>"Unlock"</button>
            </div>
        </section>
    }
}

#[component]
fn Profiles<F>(profile: ReadSignal<ProfileInput>, set_profile: WriteSignal<ProfileInput>, profiles: ReadSignal<String>, run_command: F) -> impl IntoView
where
    F: Fn(&'static str, serde_json::Value, &'static str) + Copy + 'static,
{
    let update = move |field: &'static str, value: String| {
        let mut next = profile.get();
        match field {
            "id" => next.id = value,
            "tin" => next.tin = value,
            "branch_code" => next.branch_code = value,
            "taxpayer_name" => next.taxpayer_name = value,
            "rdo_code" => next.rdo_code = value,
            "registered_address" => next.registered_address = value,
            "zip_code" => next.zip_code = value,
            "email_address" => next.email_address = value,
            _ => {}
        }
        set_profile.set(next);
    };
    view! {
        <Panel title="Taxpayer Profiles">
            <div class="form-grid">
                <input placeholder="Profile ID" prop:value=move || profile.get().id on:input=move |ev| update("id", event_target_value(&ev)) />
                <input placeholder="TIN" prop:value=move || profile.get().tin on:input=move |ev| update("tin", event_target_value(&ev)) />
                <input placeholder="Branch code" prop:value=move || profile.get().branch_code on:input=move |ev| update("branch_code", event_target_value(&ev)) />
                <input placeholder="Taxpayer name" prop:value=move || profile.get().taxpayer_name on:input=move |ev| update("taxpayer_name", event_target_value(&ev)) />
                <input placeholder="RDO code" prop:value=move || profile.get().rdo_code on:input=move |ev| update("rdo_code", event_target_value(&ev)) />
                <input placeholder="Registered address" prop:value=move || profile.get().registered_address on:input=move |ev| update("registered_address", event_target_value(&ev)) />
                <input placeholder="ZIP code" prop:value=move || profile.get().zip_code on:input=move |ev| update("zip_code", event_target_value(&ev)) />
                <input placeholder="Email" prop:value=move || profile.get().email_address on:input=move |ev| update("email_address", event_target_value(&ev)) />
            </div>
            <button on:click=move |_| run_command("create_profile", json!({"profile": profile.get()}), "profiles")>"Save profile"</button>
            <button on:click=move |_| run_command("list_profiles", json!({}), "profiles")>"Refresh"</button>
            <pre>{move || profiles.get()}</pre>
        </Panel>
    }
}

#[component]
fn Form1601C<F>(form: ReadSignal<Form1601CInput>, set_form: WriteSignal<Form1601CInput>, run_command: F) -> impl IntoView
where
    F: Fn(&'static str, serde_json::Value, &'static str) + Copy + 'static,
{
    let update = move |field: &'static str, value: String| {
        let mut next = form.get();
        match field {
            "profile_id" => next.profile_id = value,
            "tin" => next.tin = value,
            "email" => next.email = value,
            "month" => next.month = value,
            "year" => next.year = value,
            "amendment_number" => next.amendment_number = value,
            "sheets" => next.sheets = value,
            "atc" => next.atc = value,
            "rdo_code" => next.rdo_code = value,
            "taxpayer_name" => next.taxpayer_name = value,
            "registered_address" => next.registered_address = value,
            "zip_code" => next.zip_code = value,
            "telephone" => next.telephone = value,
            "treaty_code" => next.treaty_code = value,
            "tax_14" => next.tax_14 = value,
            "tax_15" => next.tax_15 = value,
            "tax_16" => next.tax_16 = value,
            "tax_17" => next.tax_17 = value,
            "tax_18" => next.tax_18 = value,
            "tax_19" => next.tax_19 = value,
            "tax_20_other" => next.tax_20_other = value,
            "tax_20" => next.tax_20 = value,
            "tax_21" => next.tax_21 = value,
            "tax_22" => next.tax_22 = value,
            "tax_23" => next.tax_23 = value,
            "tax_24" => next.tax_24 = value,
            "tax_25" => next.tax_25 = value,
            "tax_26" => next.tax_26 = value,
            "tax_27" => next.tax_27 = value,
            "tax_28" => next.tax_28 = value,
            "tax_29_other" => next.tax_29_other = value,
            "tax_29" => next.tax_29 = value,
            "tax_30" => next.tax_30 = value,
            "tax_31" => next.tax_31 = value,
            "tax_32" => next.tax_32 = value,
            "tax_33" => next.tax_33 = value,
            "tax_34" => next.tax_34 = value,
            "tax_35" => next.tax_35 = value,
            "tax_36" => next.tax_36 = value,
            "payment_agency_37" => next.payment_agency_37 = value,
            "payment_number_37" => next.payment_number_37 = value,
            "payment_date_37" => next.payment_date_37 = value,
            "payment_amount_37" => next.payment_amount_37 = value,
            "schedule_total_1" => next.schedule_total_1 = value,
            "line_of_business" => next.line_of_business = value,
            _ => {}
        }
        set_form.set(next);
    };
    let update_bool = move |field: &'static str, value: bool| {
        let mut next = form.get();
        match field {
            "amended" => next.amended = value,
            "tax_withheld_agent" => next.tax_withheld_agent = value,
            "category_private" => next.category_private = value,
            "special_tax_rate" => next.special_tax_rate = value,
            _ => {}
        }
        set_form.set(next);
    };
    let generated_json = move || {
        serde_json::to_string_pretty(&form_1601c_to_json(&form.get()))
            .unwrap_or_else(|err| format!("{{\"error\":\"{err}\"}}"))
    };

    view! {
        <Panel title="1601C Form Entry">
            <p>"Enter synthetic 1601C demo values in business-friendly sections. The app generates the JSON sent to the Rust backend."</p>

            <fieldset class="card">
                <legend>"Filing period and return type"</legend>
                <div class="form-grid">
                    <label>"Profile ID"<input prop:value=move || form.get().profile_id on:input=move |ev| update("profile_id", event_target_value(&ev)) /></label>
                    <label>"Month"<input prop:value=move || form.get().month on:input=move |ev| update("month", event_target_value(&ev)) /></label>
                    <label>"Year"<input prop:value=move || form.get().year on:input=move |ev| update("year", event_target_value(&ev)) /></label>
                    <label>"Additional sheets"<input prop:value=move || form.get().sheets on:input=move |ev| update("sheets", event_target_value(&ev)) /></label>
                    <label><input type="checkbox" prop:checked=move || form.get().amended on:change=move |ev| update_bool("amended", event_target_checked(&ev)) />" Amended return"</label>
                    <label>"Amendment number"<input prop:value=move || form.get().amendment_number on:input=move |ev| update("amendment_number", event_target_value(&ev)) /></label>
                </div>
            </fieldset>

            <fieldset class="card">
                <legend>"Taxpayer details"</legend>
                <div class="form-grid">
                    <label>"TIN"<input prop:value=move || form.get().tin on:input=move |ev| update("tin", event_target_value(&ev)) /></label>
                    <label>"Email"<input prop:value=move || form.get().email on:input=move |ev| update("email", event_target_value(&ev)) /></label>
                    <label>"RDO code"<input prop:value=move || form.get().rdo_code on:input=move |ev| update("rdo_code", event_target_value(&ev)) /></label>
                    <label>"Taxpayer name"<input prop:value=move || form.get().taxpayer_name on:input=move |ev| update("taxpayer_name", event_target_value(&ev)) /></label>
                    <label>"Registered address"<input prop:value=move || form.get().registered_address on:input=move |ev| update("registered_address", event_target_value(&ev)) /></label>
                    <label>"ZIP code"<input prop:value=move || form.get().zip_code on:input=move |ev| update("zip_code", event_target_value(&ev)) /></label>
                    <label>"Telephone"<input prop:value=move || form.get().telephone on:input=move |ev| update("telephone", event_target_value(&ev)) /></label>
                </div>
            </fieldset>

            <fieldset class="card">
                <legend>"Classification"</legend>
                <div class="form-grid">
                    <label>"ATC"<input prop:value=move || form.get().atc on:input=move |ev| update("atc", event_target_value(&ev)) /></label>
                    <label>"Treaty code"<input prop:value=move || form.get().treaty_code on:input=move |ev| update("treaty_code", event_target_value(&ev)) /></label>
                    <label><input type="checkbox" prop:checked=move || form.get().tax_withheld_agent on:change=move |ev| update_bool("tax_withheld_agent", event_target_checked(&ev)) />" Tax withheld by agent"</label>
                    <label><input type="checkbox" prop:checked=move || form.get().category_private on:change=move |ev| update_bool("category_private", event_target_checked(&ev)) />" Private withholding agent"</label>
                    <label><input type="checkbox" prop:checked=move || form.get().special_tax_rate on:change=move |ev| update_bool("special_tax_rate", event_target_checked(&ev)) />" Special tax rate"</label>
                </div>
            </fieldset>

            <fieldset class="card">
                <legend>"Tax calculation lines"</legend>
                <div class="form-grid">
                    <label>"Line 14"<input prop:value=move || form.get().tax_14 on:input=move |ev| update("tax_14", event_target_value(&ev)) /></label>
                    <label>"Line 15"<input prop:value=move || form.get().tax_15 on:input=move |ev| update("tax_15", event_target_value(&ev)) /></label>
                    <label>"Line 16"<input prop:value=move || form.get().tax_16 on:input=move |ev| update("tax_16", event_target_value(&ev)) /></label>
                    <label>"Line 17"<input prop:value=move || form.get().tax_17 on:input=move |ev| update("tax_17", event_target_value(&ev)) /></label>
                    <label>"Line 18"<input prop:value=move || form.get().tax_18 on:input=move |ev| update("tax_18", event_target_value(&ev)) /></label>
                    <label>"Line 19"<input prop:value=move || form.get().tax_19 on:input=move |ev| update("tax_19", event_target_value(&ev)) /></label>
                    <label>"Line 20 other"<input prop:value=move || form.get().tax_20_other on:input=move |ev| update("tax_20_other", event_target_value(&ev)) /></label>
                    <label>"Line 20"<input prop:value=move || form.get().tax_20 on:input=move |ev| update("tax_20", event_target_value(&ev)) /></label>
                    <label>"Line 21"<input prop:value=move || form.get().tax_21 on:input=move |ev| update("tax_21", event_target_value(&ev)) /></label>
                    <label>"Line 22"<input prop:value=move || form.get().tax_22 on:input=move |ev| update("tax_22", event_target_value(&ev)) /></label>
                    <label>"Line 23"<input prop:value=move || form.get().tax_23 on:input=move |ev| update("tax_23", event_target_value(&ev)) /></label>
                    <label>"Line 24"<input prop:value=move || form.get().tax_24 on:input=move |ev| update("tax_24", event_target_value(&ev)) /></label>
                    <label>"Line 25"<input prop:value=move || form.get().tax_25 on:input=move |ev| update("tax_25", event_target_value(&ev)) /></label>
                    <label>"Line 26"<input prop:value=move || form.get().tax_26 on:input=move |ev| update("tax_26", event_target_value(&ev)) /></label>
                    <label>"Line 27"<input prop:value=move || form.get().tax_27 on:input=move |ev| update("tax_27", event_target_value(&ev)) /></label>
                    <label>"Line 28"<input prop:value=move || form.get().tax_28 on:input=move |ev| update("tax_28", event_target_value(&ev)) /></label>
                    <label>"Line 29 other"<input prop:value=move || form.get().tax_29_other on:input=move |ev| update("tax_29_other", event_target_value(&ev)) /></label>
                    <label>"Line 29"<input prop:value=move || form.get().tax_29 on:input=move |ev| update("tax_29", event_target_value(&ev)) /></label>
                    <label>"Line 30"<input prop:value=move || form.get().tax_30 on:input=move |ev| update("tax_30", event_target_value(&ev)) /></label>
                    <label>"Line 31"<input prop:value=move || form.get().tax_31 on:input=move |ev| update("tax_31", event_target_value(&ev)) /></label>
                    <label>"Line 32"<input prop:value=move || form.get().tax_32 on:input=move |ev| update("tax_32", event_target_value(&ev)) /></label>
                    <label>"Line 33"<input prop:value=move || form.get().tax_33 on:input=move |ev| update("tax_33", event_target_value(&ev)) /></label>
                    <label>"Line 34"<input prop:value=move || form.get().tax_34 on:input=move |ev| update("tax_34", event_target_value(&ev)) /></label>
                    <label>"Line 35"<input prop:value=move || form.get().tax_35 on:input=move |ev| update("tax_35", event_target_value(&ev)) /></label>
                    <label>"Line 36"<input prop:value=move || form.get().tax_36 on:input=move |ev| update("tax_36", event_target_value(&ev)) /></label>
                </div>
            </fieldset>

            <fieldset class="card">
                <legend>"Payment/credits"</legend>
                <div class="form-grid">
                    <label>"Payment agency 37"<input prop:value=move || form.get().payment_agency_37 on:input=move |ev| update("payment_agency_37", event_target_value(&ev)) /></label>
                    <label>"Payment number 37"<input prop:value=move || form.get().payment_number_37 on:input=move |ev| update("payment_number_37", event_target_value(&ev)) /></label>
                    <label>"Payment date 37"<input prop:value=move || form.get().payment_date_37 on:input=move |ev| update("payment_date_37", event_target_value(&ev)) /></label>
                    <label>"Payment amount 37"<input prop:value=move || form.get().payment_amount_37 on:input=move |ev| update("payment_amount_37", event_target_value(&ev)) /></label>
                    <label>"Schedule total 1"<input prop:value=move || form.get().schedule_total_1 on:input=move |ev| update("schedule_total_1", event_target_value(&ev)) /></label>
                    <label>"Line of business"<input prop:value=move || form.get().line_of_business on:input=move |ev| update("line_of_business", event_target_value(&ev)) /></label>
                </div>
            </fieldset>

            <div class="actions">
                <button on:click=move |_| {
                    let input = form_1601c_to_json(&form.get());
                    run_command("render_1601c", json!({"input": input}), "package")
                }>"Render plaintext preview"</button>
                <button on:click=move |_| {
                    let input = form_1601c_to_json(&form.get());
                    run_command("package_1601c", json!({"input": input}), "package")
                }>"Package dry-run"</button>
                <button on:click=move |_| {
                    let input = form_1601c_to_json(&form.get());
                    run_command("queue_1601c_dry_run", json!({"input": input}), "jobs")
                }>"Queue dry-run job"</button>
            </div>

            <details class="card">
                <summary>"Generated JSON audit panel"</summary>
                <pre>{generated_json}</pre>
            </details>
        </Panel>
    }
}

#[component]
fn PackagePreview(
    plaintext_preview: ReadSignal<String>,
    package_preview: ReadSignal<Option<PackagePreviewResponse>>,
) -> impl IntoView {
    view! {
        <Panel title="Package Preview">
            <h3>"Plaintext XML preview"</h3>
            <pre>{move || plaintext_preview.get()}</pre>
            <div class="checklist-card">
                <h3>"Verification checklist"</h3>
                <ul class="verification-checklist">
                    <li>"Confirm the filing period and return type match the source documents."</li>
                    <li>"Confirm TIN, branch code, RDO, taxpayer name, and address are correct."</li>
                    <li>"Confirm ATC, withholding-agent category, and tax-withheld selection are correct."</li>
                    <li>"Confirm tax lines, payment credits, and schedule total are ready before encryption."</li>
                </ul>
            </div>
            <h3>"Package details"</h3>
            {move || match package_preview.get() {
                Some(package) => {
                    let manifest = package.manifest;
                    view! {
                        <dl class="details">
                            <dt>"Filename"</dt><dd>{manifest.filename}</dd>
                            <dt>"Remote path"</dt><dd>{manifest.remote_path}</dd>
                            <dt>"Period"</dt><dd>{manifest.period_mm_yyyy}</dd>
                            <dt>"Payload size"</dt><dd>{format!("{} bytes", manifest.payload_size)}</dd>
                            <dt>"Encrypted payload SHA-256"</dt><dd><code>{package.payload_sha256_short}</code><br/><span class="muted hash-full">{manifest.payload_sha256}</span></dd>
                            <dt>"Payload path"</dt><dd>{package.payload_path}</dd>
                        </dl>
                    }.into_view()
                }
                None => view! {
                    <div>
                        <p class="muted">"No package details available yet. Click Package dry-run from the 1601C screen."</p>
                    </div>
                }.into_view(),
            }}
        </Panel>
    }
}

#[component]
fn Jobs<F>(jobs: ReadSignal<String>, run_command: F) -> impl IntoView
where
    F: Fn(&'static str, serde_json::Value, &'static str) + Copy + 'static,
{
    view! {
        <Panel title="Jobs">
            <button on:click=move |_| run_command("list_jobs", json!({}), "jobs")>"Refresh jobs"</button>
            <button on:click=move |_| run_command("run_queue_dry_run", json!({"limit": 10}), "jobs")>"Run dry-run queue"</button>
            <div class="record-list">
                {move || render_jobs(&jobs.get())}
            </div>
        </Panel>
    }
}

#[component]
fn Submissions<F>(submissions: ReadSignal<String>, run_command: F) -> impl IntoView
where
    F: Fn(&'static str, serde_json::Value, &'static str) + Copy + 'static,
{
    view! {
        <Panel title="Submissions">
            <button on:click=move |_| run_command("list_submissions", json!({}), "submissions")>"Refresh submissions"</button>
            <div class="record-list">
                {move || render_submissions(&submissions.get())}
            </div>
        </Panel>
    }
}

fn parse_response_list<T>(raw: &str) -> Result<Vec<T>, serde_json::Error>
where
    T: for<'de> Deserialize<'de>,
{
    match serde_json::from_str::<Vec<T>>(raw) {
        Ok(items) => Ok(items),
        Err(list_err) => serde_json::from_str::<T>(raw)
            .map(|item| vec![item])
            .map_err(|_| list_err),
    }
}

fn latest_submission_filename(raw: &str) -> Option<String> {
    parse_response_list::<SafeSubmissionRecordResponse>(raw)
        .ok()?
        .into_iter()
        .max_by_key(|record| record.updated_unix_seconds)
        .map(|record| record.filename)
}

fn render_jobs(raw: &str) -> View {
    match parse_response_list::<JobResponse>(raw) {
        Ok(jobs) if jobs.is_empty() => view! { <p class="muted">"No jobs queued."</p> }.into_view(),
        Ok(jobs) => jobs
            .into_iter()
            .map(|job| {
                let raw_json = serde_json::to_string_pretty(&job).unwrap_or_else(|_| raw.to_string());
                let next_run = if job.next_attempt_unix_seconds == 0 {
                    "Ready now".to_string()
                } else {
                    format!("Unix {}", job.next_attempt_unix_seconds)
                };
                view! {
                    <article class="record-card">
                        <div class="record-header">
                            <strong>{format!("Job #{}", job.id)}</strong>
                            <span class="badge info">{job.status.clone()}</span>
                        </div>
                        <dl class="details compact">
                            <dt>"Job ID"</dt><dd>{job.id}</dd>
                            <dt>"Form"</dt><dd>{job.form_code}</dd>
                            <dt>"Mode"</dt><dd>{job.mode}</dd>
                            <dt>"Status"</dt><dd>{job.status}</dd>
                            <dt>"Attempts"</dt><dd>{format!("{} / {}", job.attempts, job.max_attempts)}</dd>
                            <dt>"Next run time"</dt><dd>{next_run}</dd>
                            <dt>"Last error"</dt><dd>{job.last_error.unwrap_or_else(|| "—".to_string())}</dd>
                        </dl>
                        <details class="raw-json">
                            <summary>"Raw JSON"</summary>
                            <pre>{raw_json}</pre>
                        </details>
                    </article>
                }
            })
            .collect_view()
            .into_view(),
        Err(_) => view! {
            <details class="raw-json" open>
                <summary>"Raw jobs response (could not parse typed response)"</summary>
                <pre>{raw.to_string()}</pre>
            </details>
        }
        .into_view(),
    }
}

fn render_submissions(raw: &str) -> View {
    match parse_response_list::<SafeSubmissionRecordResponse>(raw) {
        Ok(records) if records.is_empty() => view! { <p class="muted">"No submissions recorded."</p> }.into_view(),
        Ok(records) => records
            .into_iter()
            .map(|record| {
                let raw_json = serde_json::to_string_pretty(&record).unwrap_or_else(|_| raw.to_string());
                let status_class = if record.status == "Confirmed" { "badge success" } else { "badge warning" };
                view! {
                    <article class="record-card">
                        <div class="record-header">
                            <strong>{record.filename.clone()}</strong>
                            <span class=status_class>{record.status.clone()}</span>
                        </div>
                        <div class="badge-row">
                            {if record.dry_run { view! { <span class="badge info">"Dry-run"</span> }.into_view() } else { view! { <span class="badge danger">"Live"</span> }.into_view() }}
                            <span class="badge">{record.form_code.clone()}</span>
                            <span class="badge">{record.period_mm_yyyy.clone()}</span>
                        </div>
                        <dl class="details compact">
                            <dt>"Filename"</dt><dd>{record.filename}</dd>
                            <dt>"Status"</dt><dd>{record.status}</dd>
                            <dt>"Remote path"</dt><dd>{record.remote_path}</dd>
                            <dt>"Payload SHA-256"</dt><dd><code>{record.payload_sha256_short}</code></dd>
                            <dt>"Attempts"</dt><dd>{record.attempts}</dd>
                            <dt>"Receipt status"</dt><dd>{record.receipt_status.unwrap_or_else(|| "—".to_string())}</dd>
                        </dl>
                        <details class="raw-json">
                            <summary>"Raw JSON"</summary>
                            <pre>{raw_json}</pre>
                        </details>
                    </article>
                }
            })
            .collect_view()
            .into_view(),
        Err(_) => view! {
            <details class="raw-json" open>
                <summary>"Raw submissions response (could not parse typed response)"</summary>
                <pre>{raw.to_string()}</pre>
            </details>
        }
        .into_view(),
    }
}

#[component]
fn Receipt<F>(
    receipt_text: ReadSignal<String>,
    set_receipt_text: WriteSignal<String>,
    latest_package_filename: ReadSignal<Option<String>>,
    submissions: ReadSignal<String>,
    run_command: F,
) -> impl IntoView
where
    F: Fn(&'static str, serde_json::Value, &'static str) + Copy + 'static,
{
    view! {
        <Panel title="Receipt Matching">
            <p>"Paste a synthetic receipt body. The backend parser/matcher updates submission records when a matching dry-run record exists."</p>
            <textarea prop:value=receipt_text on:input=move |ev| set_receipt_text.set(event_target_value(&ev)) />
            {move || match latest_package_filename.get() {
                Some(filename) => {
                    let fill_filename = filename.clone();
                    let match_filename = filename;
                    view! {
                    <div class="actions">
                        <button on:click=move |_| set_receipt_text.set(sample_bir_receipt_for_filename(&fill_filename))>
                            "Use generated BIR receipt for latest package"
                        </button>
                        <button on:click=move |_| {
                            let synthetic_receipt = sample_bir_receipt_for_filename(&match_filename);
                            set_receipt_text.set(synthetic_receipt.clone());
                            run_command("match_receipt", json!({"receiptText": synthetic_receipt}), "submissions");
                        }>
                            "Simulate receipt and match"
                        </button>
                    </div>
                }.into_view()
                },
                None => view! { <p class="muted">"Queue and run a dry-run job, generate a package, or refresh submissions to enable receipt simulation."</p> }.into_view(),
            }}
            <button on:click=move |_| run_command("match_receipt", json!({"receiptText": receipt_text.get()}), "submissions")>"Match receipt"</button>
            <button on:click=move |_| run_command("list_submissions", json!({}), "submissions")>"Refresh submissions"</button>
            <pre>{move || submissions.get()}</pre>
        </Panel>
    }
}

#[component]
fn Logs() -> impl IntoView {
    view! {
        <Panel title="Logs">
            <p>"Safe UI logs appear in the status bar. Raw payload debug output is intentionally not exposed in the desktop MVP."</p>
        </Panel>
    }
}

#[component]
fn Settings<T, L>(
    theme: ReadSignal<String>,
    set_theme_preference: T,
    pin: ReadSignal<String>,
    set_pin: WriteSignal<String>,
    lock_now: L,
) -> impl IntoView
where
    T: Fn(&'static str) + Copy + 'static,
    L: Fn() + Copy + 'static,
{
    view! {
        <Panel title="Settings">
            <p>"Dry-run remains the default. Live submission is not exposed in this MVP shell."</p>
            <h3>"Theme"</h3>
            <div class="theme-controls">
                <button class=move || if theme.get() == "system" { "active" } else { "" } on:click=move |_| set_theme_preference("system")>"Use system theme"</button>
                <button class=move || if theme.get() == "dark" { "active" } else { "" } on:click=move |_| set_theme_preference("dark")>"Use dark theme"</button>
                <button class=move || if theme.get() == "light" { "active" } else { "" } on:click=move |_| set_theme_preference("light")>"Use light theme"</button>
            </div>
            <h3>"Lock"</h3>
            <p class="muted">"Set a 4-digit PIN, then lock the UI. The app shows a phone-style lock screen until the PIN is entered."</p>
            <div class="pin-row">
                <input
                    type="password"
                    inputmode="numeric"
                    maxlength="4"
                    placeholder="4-digit PIN"
                    prop:value=pin
                    on:input=move |ev| set_pin.set(four_digit_pin(event_target_value(&ev)))
                />
                <button on:click=move |_| lock_now()>"Lock app"</button>
            </div>
        </Panel>
    }
}

#[component]
fn Panel(title: &'static str, children: Children) -> impl IntoView {
    view! { <section class="panel"><h2>{title}</h2>{children()}</section> }
}

fn normalize_theme(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "light" => Some("light"),
        "dark" => Some("dark"),
        "system" => Some("system"),
        _ => None,
    }
}

fn four_digit_pin(value: String) -> String {
    value.chars().filter(|ch| ch.is_ascii_digit()).take(4).collect()
}

fn tin_part(digits: &str, start: usize, len: usize) -> String {
    digits.chars().skip(start).take(len).collect()
}

fn tin_branch(digits: &str) -> String {
    let mut branch: String = digits.chars().skip(9).take(5).collect();
    while branch.len() < 5 {
        branch.push('0');
    }
    branch
}

fn form_1601c_to_json(input: &Form1601CInput) -> serde_json::Value {
    let tin_digits: String = input.tin.chars().filter(|ch| ch.is_ascii_digit()).collect();
    let tin1 = tin_part(&tin_digits, 0, 3);
    let tin2 = tin_part(&tin_digits, 3, 3);
    let tin3 = tin_part(&tin_digits, 6, 3);
    let branch = tin_branch(&tin_digits);
    let month_number = input.month.parse::<u32>().unwrap_or(1);
    let year_number = input.year.parse::<u32>().unwrap_or(2026);
    let amendment_number = input.amendment_number.parse::<u32>().unwrap_or(0);

    json!({
        "profile": {
            "tin": input.tin,
            "email": input.email,
            "profile_id": input.profile_id,
        },
        "return": {
            "period": {
                "month": month_number,
                "year": year_number,
            },
            "is_amended": input.amended,
            "amendment_number": amendment_number,
        },
        "fields": {
            "txtMonth": input.month,
            "txtYear": input.year,
            "AmendedRtn_1": input.amended.to_string(),
            "AmendedRtn_2": (!input.amended).to_string(),
            "TaxWithheld_1": input.tax_withheld_agent.to_string(),
            "TaxWithheld_2": (!input.tax_withheld_agent).to_string(),
            "txtSheets": input.sheets,
            "txtATC": input.atc,
            "txtTIN1": tin1,
            "txtTIN2": tin2,
            "txtTIN3": tin3,
            "txtBranchCode": branch,
            "txtRDOCode": input.rdo_code,
            "txtTaxpayerName": input.taxpayer_name,
            "txtAddress": input.registered_address,
            "txtAddress2": input.registered_address,
            "txtZipCode": input.zip_code,
            "txtTelNum": input.telephone,
            "CatAgent_P": input.category_private.to_string(),
            "CatAgent_G": (!input.category_private).to_string(),
            "SpecialTax_1": input.special_tax_rate.to_string(),
            "SpecialTax_2": (!input.special_tax_rate).to_string(),
            "selTreaty": input.treaty_code,
            "txtTax14": input.tax_14,
            "txtTax15": input.tax_15,
            "txtTax16": input.tax_16,
            "txtTax17": input.tax_17,
            "txtTax18": input.tax_18,
            "txtTax19": input.tax_19,
            "txt20Other": input.tax_20_other,
            "txtTax20": input.tax_20,
            "txtTax21": input.tax_21,
            "txtTax22": input.tax_22,
            "txtTax23": input.tax_23,
            "txtTax24": input.tax_24,
            "txtTax25": input.tax_25,
            "txtTax26": input.tax_26,
            "txtTax27": input.tax_27,
            "txtTax28": input.tax_28,
            "txt29Other": input.tax_29_other,
            "txtTax29": input.tax_29,
            "txtTax30": input.tax_30,
            "txtTax31": input.tax_31,
            "txtTax32": input.tax_32,
            "txtTax33": input.tax_33,
            "txtTax34": input.tax_34,
            "txtTax35": input.tax_35,
            "txtTax36": input.tax_36,
            "txtAgency37": input.payment_agency_37,
            "txtNumber37": input.payment_number_37,
            "txtDate37": input.payment_date_37,
            "txtAmount37": input.payment_amount_37,
            "txtAgency38": "",
            "txtNumber38": "",
            "txtDate38": "",
            "txtAmount38": "",
            "txtNumber39": "",
            "txtDate39": "",
            "txtAmount39": "",
            "txtParticular40": "",
            "txtAgency40": "",
            "txtNumber40": "",
            "txtDate40": "",
            "txtAmount40": "",
            "txtPg2TIN1": "123",
            "txtPg2TIN2": "123",
            "txtPg2TIN3": "123",
            "txtPg2BranchCode": branch,
            "txtPg2TaxpayerName": "REDACTED TEST NAME",
            "sched1:txtTotal1": input.schedule_total_1,
            "txtCurrentPage": "1",
            "txtMaxPage": "2",
            "txtLineBus": input.line_of_business,
        }
    })
}

fn sample_receipt_text() -> String {
    sample_bir_receipt_for_filename("12345678900000-1601Cv2018-062026V1.xml")
}

fn sample_bir_receipt_for_filename(filename: &str) -> String {
    format!(
        "SUBJECT: \"Tax Return Receipt Confirmation\"\nFROM: ebirforms-noreply@bir.gov.ph\nThis confirms receipt of your submission with the following details subject to validation by BIR:\nFile name: {filename}\nDate received by BIR: 15 April 2026\nTime received by BIR: 03:10 PM\nPenalties may be imposed for any violation of the provisions of the NIRC and issuances thereof.\nFOR RETURNS WITH TAX PAYABLE:\nPlease pay through any of the following ePayment Channels:\nLand Bank of the Philippines Link.BizPortal\nLBP ATM Cards\nBancnet ATM/Debit Cards\nPCHC PayGate or PESONeT (RCBC, Robinsons Bank, UnionBank, PSBank, BPI, Asia United Bank)\nDBP PayTax Online\nCredit Cards (MasterCard/Visa)\nBancnet ATM/Debit Cards\nUnionbank of the Philippines\nUnionbank Online (for Unionbank Individual and Corporate Account Holders)\nUPAY via InstaPay (For Individual Non-Unionbank Account Holders)\nTaxpayer Agent/ Tax Software Provider-TSP\n(Gcash/PayMaya/MyEG)\nThis is a system-generated email. Please do not reply.\nBureau of Internal Revenue"
    )
}
