#![recursion_limit = "512"]

use leptos::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen(module = "/src/tauri.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

const FORM_1601C: &str = include_str!("../../../../tests/fixtures/1601C/input.json");
const FORM_2000: &str = include_str!("../../../../tests/fixtures/2000/input.json");
const FORM_2550Q: &str = include_str!("../../../../tests/fixtures/2550Q/input.json");
const FORM_0619E: &str = include_str!("../../../../tests/fixtures/0619E/input.json");
const FORM_1601EQ: &str = include_str!("../../../../tests/fixtures/1601EQ/input.json");
const FORM_1702Q: &str = include_str!("../../../../tests/fixtures/1702Q/input.json");

#[derive(Clone, Copy, Debug)]
struct TaxFormOption {
    code: &'static str,
    name: &'static str,
    frequency: &'static str,
    sample_input: &'static str,
}

const TAX_FORMS: &[TaxFormOption] = &[
    TaxFormOption { code: "1601C", name: "Monthly Remittance Return of Income Taxes Withheld on Compensation", frequency: "Monthly", sample_input: FORM_1601C },
    TaxFormOption { code: "2000", name: "Documentary Stamp Tax Declaration/Return", frequency: "Monthly", sample_input: FORM_2000 },
    TaxFormOption { code: "2550Q", name: "Quarterly Value-Added Tax Return", frequency: "Quarterly", sample_input: FORM_2550Q },
    TaxFormOption { code: "0619E", name: "Monthly Remittance Form of Creditable Income Taxes Withheld (Expanded)", frequency: "Monthly", sample_input: FORM_0619E },
    TaxFormOption { code: "1601EQ", name: "Quarterly Remittance Return of Creditable Income Taxes Withheld (Expanded)", frequency: "Quarterly", sample_input: FORM_1601EQ },
    TaxFormOption { code: "1702Q", name: "Quarterly Income Tax Return for Corporations", frequency: "Quarterly", sample_input: FORM_1702Q },
];

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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TaxpayerProfileResponse {
    profile_id: String,
    tin: String,
    email: String,
    taxpayer_name: String,
    rdo_code: Option<String>,
    registered_address: Option<String>,
    zip_code: Option<String>,
    created_unix_seconds: u64,
    updated_unix_seconds: u64,
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
    let (status, set_status) = create_signal("Ready. Create or choose a profile, then open a form from the Tax Form Library.".to_string());
    let (theme, set_theme) = create_signal("system".to_string());
    let (locked, set_locked) = create_signal(false);
    let (unlock_pin, set_unlock_pin) = create_signal(String::new());
    let (settings_pin, set_settings_pin) = create_signal(String::new());
    let (profiles, set_profiles) = create_signal(Vec::<TaxpayerProfileResponse>::new());
    let (active_profile_id, set_active_profile_id) = create_signal(None::<String>);
    let (profile, set_profile) = create_signal(ProfileInput {
        id: "synthetic-demo-profile".into(),
        tin: "123-456-789-00000".into(),
        branch_code: "00000".into(),
        taxpayer_name: "Synthetic Taxpayer Inc.".into(),
        rdo_code: "044".into(),
        registered_address: "Synthetic Address, Taguig City NCR".into(),
        zip_code: "0000".into(),
        email_address: "authorized@example.test".into(),
    });

    let initial_form = TAX_FORMS[0];
    let (selected_form, set_selected_form) = create_signal(initial_form.code.to_string());
    let (form_input_text, set_form_input_text) = create_signal(initial_form.sample_input.to_string());
    let (saved_form_input_text, set_saved_form_input_text) = create_signal(initial_form.sample_input.to_string());
    let (form_locked, set_form_locked) = create_signal(false);
    let (plaintext_preview, set_plaintext_preview) = create_signal("Validate a form to preview the plaintext XML.".to_string());
    let (package_preview, set_package_preview) = create_signal(None::<PackagePreviewResponse>);
    let (jobs, set_jobs) = create_signal(Vec::<JobResponse>::new());
    let (submissions, set_submissions) = create_signal(Vec::<SafeSubmissionRecordResponse>::new());
    let (receipt_text, set_receipt_text) = create_signal(sample_bir_receipt_for_filename("12345678900000-1601C-062026#authorized@example.test#.xml"));
    let (final_copy_confirmed, set_final_copy_confirmed) = create_signal(false);
    let (waiting_for_receipt, set_waiting_for_receipt) = create_signal(false);

    spawn_local(async move {
        match invoke_json("app_snapshot", json!({})).await {
            Ok(snapshot) => {
                if let Some(saved_theme) = snapshot
                    .get("settings")
                    .and_then(|settings| settings.get("theme"))
                    .and_then(|theme| theme.as_str())
                    .and_then(normalize_theme)
                {
                    set_theme.set(saved_theme.to_string());
                }
                let loaded_profiles: Vec<TaxpayerProfileResponse> = serde_json::from_value(snapshot.get("profiles").cloned().unwrap_or_else(|| json!([]))).unwrap_or_default();
                if active_profile_id.get_untracked().is_none() {
                    if let Some(first) = loaded_profiles.first() {
                        set_active_profile_id.set(Some(first.profile_id.clone()));
                    }
                }
                set_profiles.set(loaded_profiles);
            }
            Err(msg) => set_status.set(format!("app_snapshot failed: {msg}")),
        }
    });

    let active_profile = move || {
        let id = active_profile_id.get();
        profiles.get().into_iter().find(|p| Some(p.profile_id.clone()) == id)
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
            match invoke_json("update_settings", json!({"theme": next_theme})).await {
                Ok(value) => {
                    let saved_theme = value
                        .get("theme")
                        .and_then(|theme| theme.as_str())
                        .and_then(normalize_theme)
                        .unwrap_or(theme_name);
                    set_theme.set(saved_theme.to_string());
                    set_status.set(format!("Theme preference saved: {saved_theme}"));
                }
                Err(msg) => {
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
            match invoke_json("lock_init", json!({"pin": pin_value})).await {
                Ok(_) => {
                    set_unlock_pin.set(String::new());
                    set_locked.set(true);
                    set_status.set("App locked. Enter your 4-digit PIN to unlock.".to_string());
                }
                Err(msg) => set_status.set(format!("lock_init failed: {msg}")),
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
            match invoke_json("unlock_check", json!({"pin": pin_value})).await {
                Ok(value) => {
                    let ok = serde_json::from_value::<bool>(value).unwrap_or(false);
                    if ok {
                        set_unlock_pin.set(String::new());
                        set_locked.set(false);
                        set_status.set("Unlocked.".to_string());
                    } else {
                        set_status.set("Incorrect PIN.".to_string());
                    }
                }
                Err(msg) => set_status.set(format!("unlock_check failed: {msg}")),
            }
        });
    };

    view! {
        <main class=move || format!("app theme-{}", theme.get())>
            <aside class="sidebar">
                <div>
                    <h1>"eBIRForms"</h1>
                    <p class="muted">"Synthetic desktop filing demo"</p>
                    <nav>
                        <button on:click=move |_| set_route.set("Dashboard".to_string())>"Dashboard"</button>
                        <button on:click=move |_| set_route.set("Profiles".to_string())>"Profiles"</button>
                        <button on:click=move |_| set_route.set("Settings".to_string())>"Settings"</button>
                    </nav>
                </div>
                <div class="active-profile">
                    <span class="muted">"Active profile"</span>
                    {move || match active_profile() {
                        Some(profile) => view! {
                            <div>
                                <strong>{profile.taxpayer_name}</strong>
                                <small>{format!("{} · {}", profile.tin, profile.email)}</small>
                            </div>
                        }.into_view(),
                        None => view! { <small>"No saved profile yet"</small> }.into_view(),
                    }}
                </div>
            </aside>
            <section class="content">
                <div class="status">{move || status.get()}</div>
                {move || if locked.get() {
                    view! { <LockScreen pin=unlock_pin set_pin=set_unlock_pin unlock_app=unlock_app /> }.into_view()
                } else { match route.get().as_str() {
                    "Profiles" => view! { <Profiles profile=profile set_profile=set_profile profiles=profiles set_profiles=set_profiles set_active_profile_id=set_active_profile_id set_status=set_status /> }.into_view(),
                    "Settings" => view! { <Settings theme=theme set_theme_preference=set_theme_preference pin=settings_pin set_pin=set_settings_pin lock_now=lock_now /> }.into_view(),
                    "TaxFormFlow" => view! { <TaxFormFlow active_profile_id=active_profile_id set_route=set_route selected_form=selected_form form_input_text=form_input_text set_form_input_text=set_form_input_text saved_form_input_text=saved_form_input_text set_saved_form_input_text=set_saved_form_input_text form_locked=form_locked set_form_locked=set_form_locked plaintext_preview=plaintext_preview set_plaintext_preview=set_plaintext_preview package_preview=package_preview set_package_preview=set_package_preview jobs=jobs set_jobs=set_jobs submissions=submissions set_submissions=set_submissions receipt_text=receipt_text set_receipt_text=set_receipt_text final_copy_confirmed=final_copy_confirmed set_final_copy_confirmed=set_final_copy_confirmed waiting_for_receipt=waiting_for_receipt set_waiting_for_receipt=set_waiting_for_receipt set_status=set_status /> }.into_view(),
                    _ => view! { <Dashboard profiles=profiles active_profile_id=active_profile_id set_route=set_route selected_form=selected_form set_selected_form=set_selected_form set_form_input_text=set_form_input_text set_saved_form_input_text=set_saved_form_input_text set_form_locked=set_form_locked set_plaintext_preview=set_plaintext_preview set_package_preview=set_package_preview set_final_copy_confirmed=set_final_copy_confirmed set_waiting_for_receipt=set_waiting_for_receipt set_status=set_status /> }.into_view(),
                }}}
            </section>
        </main>
    }
}

#[component]
fn Profiles(
    profile: ReadSignal<ProfileInput>,
    set_profile: WriteSignal<ProfileInput>,
    profiles: ReadSignal<Vec<TaxpayerProfileResponse>>,
    set_profiles: WriteSignal<Vec<TaxpayerProfileResponse>>,
    set_active_profile_id: WriteSignal<Option<String>>,
    set_status: WriteSignal<String>,
) -> impl IntoView {
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

    let save_profile = move || {
        let p = profile.get_untracked();
        set_status.set("Saving profile…".to_string());
        spawn_local(async move {
            let args = json!({
                "profile": {
                    "profile_id": p.id,
                    "tin": p.tin,
                    "email": p.email_address,
                    "taxpayer_name": p.taxpayer_name,
                    "rdo_code": p.rdo_code,
                    "registered_address": p.registered_address,
                    "zip_code": p.zip_code,
                }
            });
            match invoke_json("create_profile", args).await {
                Ok(value) => match serde_json::from_value::<TaxpayerProfileResponse>(value) {
                    Ok(saved) => {
                        set_active_profile_id.set(Some(saved.profile_id.clone()));
                        set_profiles.update(|items| {
                            items.retain(|existing| existing.profile_id != saved.profile_id);
                            items.push(saved);
                        });
                        set_status.set("Profile saved and set active.".to_string());
                    }
                    Err(err) => set_status.set(format!("profile parse failed: {err}")),
                },
                Err(msg) => set_status.set(format!("create_profile failed: {msg}")),
            }
        });
    };

    view! {
        <Panel title="Profiles">
            <p>"Create or choose the active taxpayer profile. The active profile is shown at the bottom-left of the sidebar."</p>
            <div class="form-grid">
                <label>"Profile ID"<input prop:value=move || profile.get().id on:input=move |ev| update("id", event_target_value(&ev)) /></label>
                <label>"TIN"<input prop:value=move || profile.get().tin on:input=move |ev| update("tin", event_target_value(&ev)) /></label>
                <label>"Branch code"<input prop:value=move || profile.get().branch_code on:input=move |ev| update("branch_code", event_target_value(&ev)) /></label>
                <label>"Taxpayer name"<input prop:value=move || profile.get().taxpayer_name on:input=move |ev| update("taxpayer_name", event_target_value(&ev)) /></label>
                <label>"RDO code"<input prop:value=move || profile.get().rdo_code on:input=move |ev| update("rdo_code", event_target_value(&ev)) /></label>
                <label>"Registered address"<input prop:value=move || profile.get().registered_address on:input=move |ev| update("registered_address", event_target_value(&ev)) /></label>
                <label>"ZIP code"<input prop:value=move || profile.get().zip_code on:input=move |ev| update("zip_code", event_target_value(&ev)) /></label>
                <label>"Email"<input prop:value=move || profile.get().email_address on:input=move |ev| update("email_address", event_target_value(&ev)) /></label>
            </div>
            <button on:click=move |_| save_profile()>"Save profile"</button>
            <div class="record-list">
                {move || profiles.get().into_iter().map(|p| {
                    let id = p.profile_id.clone();
                    view! {
                        <article class="record-card">
                            <div class="record-header"><strong>{p.taxpayer_name}</strong><span class="badge">{p.profile_id.clone()}</span></div>
                            <p class="muted">{format!("{} · {}", p.tin, p.email)}</p>
                            <button on:click=move |_| set_active_profile_id.set(Some(id.clone()))>"Use this profile"</button>
                        </article>
                    }
                }).collect_view()}
            </div>
        </Panel>
    }
}


#[allow(clippy::too_many_arguments)]
#[component]
fn Dashboard(
    profiles: ReadSignal<Vec<TaxpayerProfileResponse>>,
    active_profile_id: ReadSignal<Option<String>>,
    set_route: WriteSignal<String>,
    selected_form: ReadSignal<String>,
    set_selected_form: WriteSignal<String>,
    set_form_input_text: WriteSignal<String>,
    set_saved_form_input_text: WriteSignal<String>,
    set_form_locked: WriteSignal<bool>,
    set_plaintext_preview: WriteSignal<String>,
    set_package_preview: WriteSignal<Option<PackagePreviewResponse>>,
    set_final_copy_confirmed: WriteSignal<bool>,
    set_waiting_for_receipt: WriteSignal<bool>,
    set_status: WriteSignal<String>,
) -> impl IntoView {
    let has_active_profile = move || {
        let id = active_profile_id.get();
        profiles
            .get()
            .into_iter()
            .any(|profile| Some(profile.profile_id) == id)
    };

    let open_form = move |code: &'static str| {
        if !has_active_profile() {
            set_status.set("Create a taxpayer profile, save it, then return to the Tax Form Library to create a form.".to_string());
            set_route.set("Profiles".to_string());
            return;
        }
        if let Some(option) = form_option(code) {
            set_selected_form.set(option.code.to_string());
            set_form_input_text.set(option.sample_input.to_string());
            set_saved_form_input_text.set(option.sample_input.to_string());
            set_form_locked.set(false);
            set_plaintext_preview.set("Validate a form to preview the plaintext XML.".to_string());
            set_package_preview.set(None);
            set_final_copy_confirmed.set(false);
            set_waiting_for_receipt.set(false);
            set_status.set(format!("Opened BIR Form {}.", option.code));
            set_route.set("TaxFormFlow".to_string());
        }
    };

    view! {
        <section class="dashboard-library">
            <Panel title="Tax Form Library">
                {move || if has_active_profile() {
                    view! { <p>"Choose a tax form to create a filing flow for the active taxpayer profile."</p> }.into_view()
                } else {
                    view! {
                        <div class="alert warning">
                            "Create a taxpayer profile first. Save it in Profiles before creating a tax form."
                            <div class="actions"><button on:click=move |_| set_route.set("Profiles".to_string())>"Create profile"</button></div>
                        </div>
                    }.into_view()
                }}
                <div class="form-library">
                    {TAX_FORMS.iter().map(|option| {
                        let code = option.code;
                        view! {
                            <button
                                class=move || if selected_form.get() == code { "form-tile active" } else { "form-tile" }
                                disabled=move || !has_active_profile()
                                title=move || if has_active_profile() { "Open tax form flow" } else { "Create and save a taxpayer profile first" }
                                on:click=move |_| open_form(code)
                            >
                                <strong>{option.code}</strong>
                                <span>{option.name}</span>
                                <small>{option.frequency}</small>
                            </button>
                        }
                    }).collect_view()}
                </div>
            </Panel>
        </section>
    }
}

#[allow(clippy::too_many_arguments)]
#[component]
fn TaxFormFlow(
    active_profile_id: ReadSignal<Option<String>>,
    set_route: WriteSignal<String>,
    selected_form: ReadSignal<String>,
    form_input_text: ReadSignal<String>,
    set_form_input_text: WriteSignal<String>,
    saved_form_input_text: ReadSignal<String>,
    set_saved_form_input_text: WriteSignal<String>,
    form_locked: ReadSignal<bool>,
    set_form_locked: WriteSignal<bool>,
    plaintext_preview: ReadSignal<String>,
    set_plaintext_preview: WriteSignal<String>,
    package_preview: ReadSignal<Option<PackagePreviewResponse>>,
    set_package_preview: WriteSignal<Option<PackagePreviewResponse>>,
    jobs: ReadSignal<Vec<JobResponse>>,
    set_jobs: WriteSignal<Vec<JobResponse>>,
    submissions: ReadSignal<Vec<SafeSubmissionRecordResponse>>,
    set_submissions: WriteSignal<Vec<SafeSubmissionRecordResponse>>,
    receipt_text: ReadSignal<String>,
    set_receipt_text: WriteSignal<String>,
    final_copy_confirmed: ReadSignal<bool>,
    set_final_copy_confirmed: WriteSignal<bool>,
    waiting_for_receipt: ReadSignal<bool>,
    set_waiting_for_receipt: WriteSignal<bool>,
    set_status: WriteSignal<String>,
) -> impl IntoView {
    let save_form = move || {
        set_saved_form_input_text.set(form_input_text.get_untracked());
        set_status.set("Saved current form changes locally in the demo session.".to_string());
    };

    let edit_form = move || {
        set_form_locked.set(false);
        set_final_copy_confirmed.set(false);
        set_waiting_for_receipt.set(false);
        set_status.set("Form reopened for editing; final-copy confirmation was cleared.".to_string());
    };

    let validate_form = move || {
        if active_profile_id.get_untracked().is_none() {
            set_status.set("Create and save a taxpayer profile before validating a tax form.".to_string());
            return;
        }
        let form_code = selected_form.get_untracked();
        let input_text = form_input_text.get_untracked();
        let Ok(input_json) = serde_json::from_str::<Value>(&input_text) else {
            set_status.set("Validate failed: form JSON is invalid.".to_string());
            return;
        };
        set_status.set(format!("Validating and encrypting BIR Form {form_code}…"));
        spawn_local(async move {
            let render_args = json!({"formCode": form_code, "input": input_json});
            match invoke_json("render_tax_form", render_args).await {
                Ok(rendered) => {
                    let plaintext = rendered.as_str().unwrap_or_default().to_string();
                    set_plaintext_preview.set(plaintext);
                }
                Err(msg) => {
                    set_status.set(format!("Validate failed while rendering: {msg}"));
                    return;
                }
            }
            let package_args = json!({"formCode": form_code, "input": input_json});
            match invoke_json("package_tax_form", package_args).await {
                Ok(value) => match serde_json::from_value::<PackagePreviewResponse>(value) {
                    Ok(package) => {
                        set_receipt_text.set(sample_bir_receipt_for_filename(&package.manifest.filename));
                        set_saved_form_input_text.set(input_text.clone());
                        set_package_preview.set(Some(package));
                        set_form_locked.set(true);
                        set_final_copy_confirmed.set(false);
                        set_waiting_for_receipt.set(false);
                        set_status.set("Validated. Form is locked; review package details, then confirm before Submit Final Copy.".to_string());
                    }
                    Err(err) => set_status.set(format!("Package parse failed: {err}")),
                },
                Err(msg) => set_status.set(format!("Package failed: {msg}")),
            }
        });
    };

    let queue_dry_run = move || {
        if active_profile_id.get_untracked().is_none() {
            set_status.set("Create and save a taxpayer profile before queueing a tax form.".to_string());
            return;
        }
        let form_code = selected_form.get_untracked();
        let Ok(input_json) = serde_json::from_str::<Value>(&saved_form_input_text.get_untracked()) else {
            set_status.set("Queue failed: saved form JSON is invalid.".to_string());
            return;
        };
        set_status.set("Queueing dry-run job…".to_string());
        spawn_local(async move {
            match invoke_json("queue_tax_form_dry_run", json!({"formCode": form_code, "input": input_json})).await {
                Ok(_) => refresh_jobs_and_submissions(set_jobs, set_submissions, set_status).await,
                Err(msg) => set_status.set(format!("queue_tax_form_dry_run failed: {msg}")),
            }
        });
    };

    let run_queue = move || {
        set_status.set("Running dry-run queue…".to_string());
        spawn_local(async move {
            match invoke_json("run_queue_dry_run", json!({"limit": 10})).await {
                Ok(_) => refresh_jobs_and_submissions(set_jobs, set_submissions, set_status).await,
                Err(msg) => set_status.set(format!("run_queue_dry_run failed: {msg}")),
            }
        });
    };

    let simulate_receipt = move || {
        let filename = package_preview
            .get_untracked()
            .map(|p| p.manifest.filename)
            .or_else(|| latest_submission_filename(&submissions.get_untracked()));
        let Some(filename) = filename else {
            set_status.set("Run Validate or the dry-run queue before simulating a receipt.".to_string());
            return;
        };
        let synthetic_receipt = sample_bir_receipt_for_filename(&filename);
        set_receipt_text.set(synthetic_receipt.clone());
        set_status.set("Matching synthetic BIR receipt…".to_string());
        spawn_local(async move {
            match invoke_json("match_receipt", json!({"receiptText": synthetic_receipt})).await {
                Ok(value) => match serde_json::from_value::<Vec<SafeSubmissionRecordResponse>>(value) {
                    Ok(records) => {
                        set_submissions.set(records);
                        set_waiting_for_receipt.set(false);
                        set_status.set("Receipt matched against submission records.".to_string());
                    }
                    Err(err) => set_status.set(format!("receipt response parse failed: {err}")),
                },
                Err(msg) => set_status.set(format!("match_receipt failed: {msg}")),
            }
        });
    };

    let submit_final_copy = move || {
        if active_profile_id.get_untracked().is_none() {
            set_status.set("Create and save a taxpayer profile before submitting a final copy.".to_string());
            return;
        }
        if !form_locked.get_untracked() || package_preview.get_untracked().is_none() {
            set_status.set("Submit Final Copy requires a fully validated form first.".to_string());
            return;
        }
        if !final_copy_confirmed.get_untracked() {
            set_status.set("Confirm the validated final copy before submitting.".to_string());
            return;
        }
        if waiting_for_receipt.get_untracked() {
            set_status.set("Already submitted. Waiting for a BIR receipt.".to_string());
            return;
        }
        let form_code = selected_form.get_untracked();
        let Ok(input_json) = serde_json::from_str::<Value>(&saved_form_input_text.get_untracked()) else {
            set_status.set("Submit Final Copy failed: validated form JSON is invalid.".to_string());
            return;
        };
        set_status.set("Submit Final Copy: queueing and running dry-run delivery…".to_string());
        spawn_local(async move {
            match invoke_json("queue_tax_form_dry_run", json!({"formCode": form_code, "input": input_json})).await {
                Ok(_) => {}
                Err(msg) => {
                    set_status.set(format!("Submit Final Copy queue failed: {msg}"));
                    return;
                }
            }
            match invoke_json("run_queue_dry_run", json!({"limit": 10})).await {
                Ok(_) => {
                    set_waiting_for_receipt.set(true);
                    refresh_jobs_and_submissions(set_jobs, set_submissions, set_status).await;
                    set_status.set("Submit Final Copy queued and ran. Waiting for a BIR receipt confirmation.".to_string());
                }
                Err(msg) => set_status.set(format!("Submit Final Copy run failed: {msg}")),
            }
        });
    };

    view! {
        <section class="form-flow-column">
            <div class="actions">
                <button on:click=move |_| set_route.set("Dashboard".to_string())>"← Tax Form Library"</button>
            </div>
            <Panel title="Tax Form Flow">
                <div class="record-header">
                    <div>
                        <h3>{move || selected_form.get()}</h3>
                        <p class="muted">{move || form_option(&selected_form.get()).map(|f| f.name).unwrap_or("Unknown form")}</p>
                    </div>
                    {move || if form_locked.get() { view! { <span class="badge success">"Validated / locked"</span> }.into_view() } else { view! { <span class="badge warning">"Editable"</span> }.into_view() }}
                </div>
                <div class="actions">
                    <button on:click=move |_| validate_form() disabled=move || waiting_for_receipt.get()>"Validate"</button>
                    <button on:click=move |_| edit_form() disabled=move || waiting_for_receipt.get()>"Edit"</button>
                    <button on:click=move |_| save_form() disabled=move || form_locked.get() || waiting_for_receipt.get()>"Save"</button>
                    <button disabled=true title="Print is not implemented in this demo">"Print"</button>
                    <button
                        on:click=move |_| submit_final_copy()
                        disabled=move || !form_locked.get() || package_preview.get().is_none() || !final_copy_confirmed.get() || waiting_for_receipt.get()
                        title="Enabled only after Validate and final-copy confirmation"
                    >"Submit Final Copy"</button>
                </div>
                <div class="checklist-card">
                    <h3>"Final copy confirmation"</h3>
                    <label class="checkbox-row">
                        <input
                            type="checkbox"
                            prop:checked=move || final_copy_confirmed.get()
                            prop:disabled=move || !form_locked.get() || package_preview.get().is_none() || waiting_for_receipt.get()
                            on:change=move |ev| set_final_copy_confirmed.set(event_target_checked(&ev))
                        />
                        <span>"I confirm the whole form is validated, locked, and ready to submit as the final copy."</span>
                    </label>
                    {move || if waiting_for_receipt.get() {
                        view! { <p class="muted">"Final copy has been queued and run. Waiting for BIR receipt confirmation."</p> }.into_view()
                    } else if form_locked.get() && package_preview.get().is_some() {
                        view! { <p class="muted">"Review package details, then tick the confirmation to enable Submit Final Copy."</p> }.into_view()
                    } else {
                        view! { <p class="muted">"Validate the form before final-copy confirmation is available."</p> }.into_view()
                    }}
                </div>
                <HumanTaxForm
                    selected_form=selected_form
                    form_input_text=form_input_text
                    set_form_input_text=set_form_input_text
                    form_locked=form_locked
                />
                <PackageDetails package_preview=package_preview />
                <div class="actions">
                    <button on:click=move |_| queue_dry_run() disabled=move || !form_locked.get() || waiting_for_receipt.get()>"Queue dry-run"</button>
                    <button on:click=move |_| run_queue() disabled=move || waiting_for_receipt.get()>"Run dry-run queue"</button>
                    <button on:click=move |_| simulate_receipt() disabled=move || package_preview.get().is_none() && submissions.get().is_empty()>"Simulate received BIR receipt"</button>
                </div>
            </Panel>

            <Panel title="Plaintext XML Preview">
                <pre>{move || plaintext_preview.get()}</pre>
            </Panel>

            <Panel title="Submission Activity">
                <h3>"Jobs"</h3>
                <div class="record-list">{move || render_jobs(jobs.get())}</div>
                <h3>"Submissions / receipt matching"</h3>
                <textarea prop:value=receipt_text on:input=move |ev| set_receipt_text.set(event_target_value(&ev)) />
                <div class="record-list">{move || render_submissions(submissions.get())}</div>
            </Panel>
        </section>
    }
}


#[component]
fn HumanTaxForm(
    selected_form: ReadSignal<String>,
    form_input_text: ReadSignal<String>,
    set_form_input_text: WriteSignal<String>,
    form_locked: ReadSignal<bool>,
) -> impl IntoView {
    view! {
        <div class="human-form">
            <div class="record-header">
                <div>
                    <h3>{move || format!("BIR Form {} data entry", selected_form.get())}</h3>
                    <p class="muted">"Human-readable fields generate the synthetic JSON payload used for XML rendering and packaging. Operators do not edit JSON directly."</p>
                </div>
            </div>
            {move || render_human_tax_form_fields(selected_form.get(), form_input_text.get(), set_form_input_text, form_locked.get())}
        </div>
    }
}

fn render_human_tax_form_fields(
    form_code: String,
    input_text: String,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let Ok(input) = serde_json::from_str::<Value>(&input_text) else {
        return view! { <div class="alert warning">"The current form data cannot be parsed. Choose a form from the Tax Form Library to reset the human form."</div> }.into_view();
    };

    if form_code == "1601C" {
        return render_1601c_physical_form(input, set_form_input_text, locked);
    }

    let profile = input.get("profile").and_then(Value::as_object).cloned().unwrap_or_default();
    let return_obj = input.get("return").and_then(Value::as_object).cloned().unwrap_or_default();
    let period = return_obj.get("period").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut field_items: Vec<(String, String)> = input
        .get("fields")
        .and_then(Value::as_object)
        .map(|fields| fields.iter().map(|(key, value)| (key.clone(), value_to_form_string(value))).collect())
        .unwrap_or_default();
    field_items.sort_by(|a, b| field_sort_key(&a.0).cmp(&field_sort_key(&b.0)));

    let profile_fields = vec![
        ("profile_id", "Profile ID"),
        ("tin", "TIN"),
        ("email", "Authorized email"),
    ];
    let period_fields = vec![
        ("month", "Month"),
        ("quarter", "Quarter"),
        ("year", "Year"),
    ];
    let return_fields = vec![
        ("is_amended", "Amended return?"),
        ("amendment_number", "Amendment number"),
    ];

    view! {
        <div class="form-section">
            <h3>{format!("Form {} filing details", form_code)}</h3>
            <div class="form-grid">
                {profile_fields.into_iter().map(|(key, label)| {
                    let value = profile.get(key).map(value_to_form_string).unwrap_or_default();
                    render_json_input("profile", key, label, value, locked, set_form_input_text)
                }).collect_view()}
                {period_fields.into_iter().filter_map(|(key, label)| {
                    period.get(key).map(|value| render_nested_json_input("return", "period", key, label, value_to_form_string(value), locked, set_form_input_text))
                }).collect_view()}
                {return_fields.into_iter().filter_map(|(key, label)| {
                    return_obj.get(key).map(|value| render_json_input("return", key, label, value_to_form_string(value), locked, set_form_input_text))
                }).collect_view()}
            </div>
        </div>
        <div class="form-section">
            <h3>"BIR line items and schedules"</h3>
            <p class="muted">"These controls cover the available numbered BIR form fields while keeping the generated JSON hidden."</p>
            <div class="form-grid dense">
                {field_items.into_iter().map(|(key, value)| {
                    let label = human_field_label(&key);
                    render_field_value_input(key, label, value, locked, set_form_input_text)
                }).collect_view()}
            </div>
        </div>
    }.into_view()
}

fn render_json_input(
    section: &'static str,
    key: &'static str,
    label: &'static str,
    value: String,
    locked: bool,
    set_form_input_text: WriteSignal<String>,
) -> View {
    let is_bool = matches!(value.as_str(), "true" | "false");
    if is_bool {
        let checked = value == "true";
        view! {
            <label class="checkbox-row field-control">
                <input
                    type="checkbox"
                    prop:checked=checked
                    prop:disabled=locked
                    on:change=move |ev| update_top_level_value(set_form_input_text, section, key, Value::Bool(event_target_checked(&ev)))
                />
                <span>{label}</span>
            </label>
        }.into_view()
    } else {
        view! {
            <label class="field-control">{label}
                <input
                    prop:value=value
                    prop:readonly=locked
                    on:input=move |ev| update_top_level_value(set_form_input_text, section, key, typed_json_value(section, key, event_target_value(&ev)))
                />
            </label>
        }.into_view()
    }
}

fn render_nested_json_input(
    section: &'static str,
    nested: &'static str,
    key: &'static str,
    label: &'static str,
    value: String,
    locked: bool,
    set_form_input_text: WriteSignal<String>,
) -> View {
    view! {
        <label class="field-control">{label}
            <input
                prop:value=value
                prop:readonly=locked
                on:input=move |ev| update_nested_value(set_form_input_text, section, nested, key, typed_json_value(nested, key, event_target_value(&ev)))
            />
        </label>
    }.into_view()
}

fn render_field_value_input(
    key: String,
    label: String,
    value: String,
    locked: bool,
    set_form_input_text: WriteSignal<String>,
) -> View {
    if matches!(value.as_str(), "true" | "false") {
        let checked = value == "true";
        let checkbox_key = key.clone();
        view! {
            <label class="checkbox-row field-control">
                <input
                    type="checkbox"
                    prop:checked=checked
                    prop:disabled=locked
                    on:change=move |ev| update_field_string(set_form_input_text, &checkbox_key, event_target_checked(&ev).to_string())
                />
                <span>{label}</span>
            </label>
        }.into_view()
    } else {
        let input_key = key.clone();
        view! {
            <label class="field-control">{label}
                <input
                    type="text"
                    prop:value=value
                    prop:readonly=locked
                    on:input=move |ev| update_field_string(set_form_input_text, &input_key, event_target_value(&ev))
                />
            </label>
        }.into_view()
    }
}

fn update_top_level_value(
    set_form_input_text: WriteSignal<String>,
    section: &str,
    key: &str,
    value: Value,
) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(obj) = root.get_mut(section).and_then(Value::as_object_mut) {
                obj.insert(key.to_string(), value);
                *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
            }
        }
    });
}

fn update_nested_value(
    set_form_input_text: WriteSignal<String>,
    section: &str,
    nested: &str,
    key: &str,
    value: Value,
) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(obj) = root
                .get_mut(section)
                .and_then(Value::as_object_mut)
                .and_then(|section| section.get_mut(nested))
                .and_then(Value::as_object_mut)
            {
                obj.insert(key.to_string(), value);
                *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
            }
        }
    });
}

fn update_field_string(set_form_input_text: WriteSignal<String>, key: &str, value: String) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                fields.insert(key.to_string(), Value::String(value));
                *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
            }
        }
    });
}

fn typed_json_value(section: &str, key: &str, value: String) -> Value {
    if matches!((section, key), ("period", "month") | ("period", "quarter") | ("period", "year") | ("return", "amendment_number")) {
        value
            .trim()
            .parse::<i64>()
            .map(|number| Value::Number(number.into()))
            .unwrap_or_else(|_| Value::String(value))
    } else {
        Value::String(value)
    }
}

fn value_to_form_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn field_sort_key(key: &str) -> String {
    let lower = key.to_ascii_lowercase();
    let priority = if lower.contains("tin") || lower.contains("taxpayer") || lower.contains("rdo") || lower.contains("address") || lower.contains("email") {
        "0"
    } else if lower.contains("month") || lower.contains("year") || lower.contains("quarter") || lower.contains("period") || lower.contains("amended") {
        "1"
    } else if lower.contains("tax") || lower.contains("total") || lower.contains("amount") || lower.contains("payment") || lower.contains("sales") || lower.contains("purchase") {
        "2"
    } else if lower.contains("sched") {
        "3"
    } else {
        "4"
    };
    format!("{priority}:{lower}")
}

fn human_field_label(key: &str) -> String {
    let cleaned = key
        .rsplit(':')
        .next()
        .unwrap_or(key)
        .trim_start_matches("txt")
        .trim_start_matches("opt")
        .trim_start_matches("chk")
        .trim_start_matches("drp");
    let mut label = String::new();
    let mut previous_was_lower = false;
    for ch in cleaned.chars() {
        if matches!(ch, '_' | '-' ) {
            label.push(' ');
            previous_was_lower = false;
        } else if ch.is_ascii_uppercase() && previous_was_lower {
            label.push(' ');
            label.push(ch);
            previous_was_lower = false;
        } else {
            label.push(ch);
            previous_was_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    let label = label.replace("No", " No. ").replace("  ", " ");
    format!("{} ({})", label.trim(), key)
}


fn render_1601c_physical_form(
    input: Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    view! {
        <div class="bir-paper form-1601c">
            <div class="bir-title-grid">
                <div class="bir-form-no"><span>"BIR Form No."</span><strong>"1601-C"</strong><small>"January 2018 (ENCS)"</small></div>
                <div class="bir-title"><strong>"Monthly Remittance Return"</strong><span>"of Income Taxes Withheld on Compensation"</span></div>
                <div class="bir-barcode">"1601-C 01/18ENCS P1"</div>
            </div>
            <div class="bir-grid top-strip">
                {render_1601c_period_box(&input, set_form_input_text, locked)}
                {render_1601c_pair_box("2", "Amended Return?", "AmendedRtn_1", "AmendedRtn_2", &input, set_form_input_text, locked)}
                {render_1601c_pair_box("3", "Any Taxes Withheld?", "TaxWithheld_1", "TaxWithheld_2", &input, set_form_input_text, locked)}
                {render_1601c_field_box("4", "Number of Sheet/s Attached", "txtSheets", &input, set_form_input_text, locked)}
                {render_1601c_field_box("5", "ATC", "txtATC", &input, set_form_input_text, locked)}
            </div>
            <div class="bir-section-title">"Part I – Background Information"</div>
            <div class="bir-grid background-grid">
                {render_1601c_tin_box(&input, set_form_input_text, locked)}
                {render_1601c_field_box("7", "RDO Code", "txtRDOCode", &input, set_form_input_text, locked)}
                {render_1601c_wide_field_box("8", "Withholding Agent’s Name", "txtTaxpayerName", &input, set_form_input_text, locked)}
                {render_1601c_wide_field_box("9", "Registered Address", "txtAddress", &input, set_form_input_text, locked)}
                {render_1601c_field_box("9A", "ZIP Code", "txtZipCode", &input, set_form_input_text, locked)}
                {render_1601c_field_box("10", "Contact Number", "txtTelNum", &input, set_form_input_text, locked)}
                {render_1601c_pair_box("11", "Category of Withholding Agent", "CatAgent_P", "CatAgent_G", &input, set_form_input_text, locked)}
                {render_1601c_profile_email_box(&input, set_form_input_text, locked)}
                {render_1601c_pair_box("13", "Payees availing of tax relief under Special Law or International Tax Treaty?", "SpecialTax_1", "SpecialTax_2", &input, set_form_input_text, locked)}
                {render_1601c_field_box("13A", "If yes, specify", "selTreaty", &input, set_form_input_text, locked)}
            </div>
            <div class="bir-section-title">"Part II – Computation of Tax"</div>
            <div class="bir-table computation-table">
                {render_1601c_amount_row("14", "Total Amount of Compensation", "txtTax14", &input, set_form_input_text, locked)}
                <div class="bir-subrow">"Less: Non-Taxable/Exempt Compensation"</div>
                {render_1601c_amount_row("15", "Statutory Minimum Wage for Minimum Wage Earners (MWEs)", "txtTax15", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("16", "Holiday Pay, Overtime Pay, Night Shift Differential Pay, Hazard Pay (for MWEs only)", "txtTax16", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("17", "13th Month Pay and Other Benefits", "txtTax17", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("18", "De Minimis Benefits", "txtTax18", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("19", "SSS, GSIS, PHIC, HDMF Mandatory Contributions & Union Dues (employee’s share only)", "txtTax19", &input, set_form_input_text, locked)}
                {render_1601c_amount_row_with_specify("20", "Other Non-Taxable Compensation", "txt20Other", "txtTax20", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("21", "Total Non-Taxable Compensation (Sum of Items 15 to 20)", "txtTax21", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("22", "Total Taxable Compensation (Item 14 Less Item 21)", "txtTax22", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("23", "Less: Taxable compensation not subject to withholding tax", "txtTax23", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("24", "Net Taxable Compensation (Item 22 Less Item 23)", "txtTax24", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("25", "Total Taxes Withheld", "txtTax25", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("26", "Add/(Less): Adjustment of Taxes Withheld from Previous Month/s (From Part IV-Schedule 1, Item 4)", "txtTax26", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("27", "Taxes Withheld for Remittance (Sum of Items 25 and 26)", "txtTax27", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("28", "Less: Tax Remitted in Return Previously Filed, if this is an amended return", "txtTax28", &input, set_form_input_text, locked)}
                {render_1601c_amount_row_with_specify("29", "Other Remittances Made", "txt29Other", "txtTax29", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("30", "Total Tax Remittances Made (Sum of Items 28 and 29)", "txtTax30", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("31", "Tax Still Due/(Over-remittance) (Item 27 Less Item 30)", "txtTax31", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("32", "Add: Penalties – Surcharge", "txtTax32", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("33", "Add: Penalties – Interest", "txtTax33", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("34", "Add: Penalties – Compromise", "txtTax34", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("35", "Total Penalties (Sum of Items 32 to 34)", "txtTax35", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("36", "TOTAL AMOUNT STILL DUE/(Over-remittance) (Sum of Items 31 and 35)", "txtTax36", &input, set_form_input_text, locked)}
            </div>
            <div class="bir-section-title">"Part III – Details of Payment"</div>
            <div class="bir-payment-grid">
                {render_1601c_payment_row("37", "Cash/Bank Debit Memo", Some("txtAgency37"), "txtNumber37", "txtDate37", "txtAmount37", &input, set_form_input_text, locked)}
                {render_1601c_payment_row("38", "Check", Some("txtAgency38"), "txtNumber38", "txtDate38", "txtAmount38", &input, set_form_input_text, locked)}
                {render_1601c_payment_row("39", "Tax Debit Memo", None, "txtNumber39", "txtDate39", "txtAmount39", &input, set_form_input_text, locked)}
                {render_1601c_payment_row("40", "Others", Some("txtAgency40"), "txtNumber40", "txtDate40", "txtAmount40", &input, set_form_input_text, locked)}
                {render_1601c_field_box("40 specify", "Other payment particulars", "txtParticular40", &input, set_form_input_text, locked)}
            </div>
            <div class="bir-section-title">"Part IV – Schedule I: Adjustment of Taxes Withheld on Compensation from Previous Months"</div>
            <div class="bir-grid background-grid">
                {render_1601c_wide_field_box("Page 2 TIN", "TIN carried to page 2", "txtPg2TIN1", &input, set_form_input_text, locked)}
                {render_1601c_wide_field_box("Page 2 name", "Withholding Agent’s Name carried to page 2", "txtPg2TaxpayerName", &input, set_form_input_text, locked)}
                {render_1601c_amount_row("Schedule I Item 4", "Total Adjustment (Sum of Items 1 to 3) – maps to Part II Item 26", "sched1:txtTotal1", &input, set_form_input_text, locked)}
            </div>
        </div>
    }.into_view()
}

fn render_1601c_period_box(input: &Value, set_form_input_text: WriteSignal<String>, locked: bool) -> View {
    let month = field_value(input, "txtMonth");
    let year = field_value(input, "txtYear");
    view! {
        <label class="bir-box">"1 For the Month (MM/YYYY)"
            <div class="split-inputs">
                <input aria-label="Month" prop:value=month prop:readonly=locked on:input=move |ev| update_1601c_period(set_form_input_text, "month", event_target_value(&ev)) />
                <span>"/"</span>
                <input aria-label="Year" prop:value=year prop:readonly=locked on:input=move |ev| update_1601c_period(set_form_input_text, "year", event_target_value(&ev)) />
            </div>
        </label>
    }.into_view()
}

fn render_1601c_tin_box(input: &Value, set_form_input_text: WriteSignal<String>, locked: bool) -> View {
    let tin1 = field_value(input, "txtTIN1");
    let tin2 = field_value(input, "txtTIN2");
    let tin3 = field_value(input, "txtTIN3");
    let branch = field_value(input, "txtBranchCode");
    view! {
        <label class="bir-box span-2">"6 Taxpayer Identification Number (TIN)"
            <div class="split-inputs tin-inputs">
                <input aria-label="TIN first block" prop:value=tin1 prop:readonly=locked on:input=move |ev| update_1601c_tin(set_form_input_text, "txtTIN1", event_target_value(&ev)) />
                <span>"/"</span>
                <input aria-label="TIN second block" prop:value=tin2 prop:readonly=locked on:input=move |ev| update_1601c_tin(set_form_input_text, "txtTIN2", event_target_value(&ev)) />
                <span>"/"</span>
                <input aria-label="TIN third block" prop:value=tin3 prop:readonly=locked on:input=move |ev| update_1601c_tin(set_form_input_text, "txtTIN3", event_target_value(&ev)) />
                <span>"/"</span>
                <input aria-label="Branch code" prop:value=branch prop:readonly=locked on:input=move |ev| update_1601c_tin(set_form_input_text, "txtBranchCode", event_target_value(&ev)) />
            </div>
        </label>
    }.into_view()
}

fn render_1601c_profile_email_box(input: &Value, set_form_input_text: WriteSignal<String>, locked: bool) -> View {
    let value = input
        .get("profile")
        .and_then(|profile| profile.get("email"))
        .map(value_to_form_string)
        .unwrap_or_default();
    view! {
        <label class="bir-box span-2">"12 Email Address"
            <input prop:value=value prop:readonly=locked on:input=move |ev| update_top_level_value(set_form_input_text, "profile", "email", Value::String(event_target_value(&ev))) />
        </label>
    }.into_view()
}

fn render_1601c_field_box(item: &'static str, label: &'static str, key: &'static str, input: &Value, set_form_input_text: WriteSignal<String>, locked: bool) -> View {
    let value = field_value(input, key);
    view! {
        <label class="bir-box">{format!("{item} {label}")}
            <input prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, key, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_1601c_wide_field_box(item: &'static str, label: &'static str, key: &'static str, input: &Value, set_form_input_text: WriteSignal<String>, locked: bool) -> View {
    let value = field_value(input, key);
    view! {
        <label class="bir-box span-2">{format!("{item} {label}")}
            <input prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, key, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_1601c_pair_box(item: &'static str, label: &'static str, yes_key: &'static str, no_key: &'static str, input: &Value, set_form_input_text: WriteSignal<String>, locked: bool) -> View {
    let yes = field_value(input, yes_key) == "true";
    let no = field_value(input, no_key) == "true";
    view! {
        <div class="bir-box checkbox-pair">
            <span>{format!("{item} {label}")}</span>
            <label><input type="checkbox" prop:checked=yes prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) { update_checkbox_pair(set_form_input_text, yes_key, no_key, true) } />"Yes"</label>
            <label><input type="checkbox" prop:checked=no prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) { update_checkbox_pair(set_form_input_text, yes_key, no_key, false) } />"No"</label>
        </div>
    }.into_view()
}

fn render_1601c_amount_row(item: &'static str, label: &'static str, key: &'static str, input: &Value, set_form_input_text: WriteSignal<String>, locked: bool) -> View {
    let value = field_value(input, key);
    view! {
        <label class="bir-row">
            <span class="item-no">{item}</span><span class="item-label">{label}</span>
            <input class="amount-input" prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, key, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_1601c_amount_row_with_specify(item: &'static str, label: &'static str, specify_key: &'static str, amount_key: &'static str, input: &Value, set_form_input_text: WriteSignal<String>, locked: bool) -> View {
    let specify = field_value(input, specify_key);
    let amount = field_value(input, amount_key);
    view! {
        <label class="bir-row specify-row">
            <span class="item-no">{item}</span><span class="item-label">{label}</span>
            <input class="specify-input" placeholder="specify" prop:value=specify prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, specify_key, event_target_value(&ev)) />
            <input class="amount-input" prop:value=amount prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, amount_key, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_1601c_payment_row(item: &'static str, label: &'static str, agency_key: Option<&'static str>, number_key: &'static str, date_key: &'static str, amount_key: &'static str, input: &Value, set_form_input_text: WriteSignal<String>, locked: bool) -> View {
    let agency = agency_key.map(|key| field_value(input, key)).unwrap_or_default();
    let number = field_value(input, number_key);
    let date = field_value(input, date_key);
    let amount = field_value(input, amount_key);
    view! {
        <div class="payment-row">
            <strong>{format!("{item} {label}")}</strong>
            <input placeholder="Drawee Bank/Agency" prop:value=agency prop:readonly=locked on:input=move |ev| if let Some(key) = agency_key { update_field_string(set_form_input_text, key, event_target_value(&ev)) } />
            <input placeholder="Number" prop:value=number prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, number_key, event_target_value(&ev)) />
            <input placeholder="Date (MM/DD/YYYY)" prop:value=date prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, date_key, event_target_value(&ev)) />
            <input class="amount-input" placeholder="Amount" prop:value=amount prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, amount_key, event_target_value(&ev)) />
        </div>
    }.into_view()
}

fn field_value(input: &Value, key: &str) -> String {
    input
        .get("fields")
        .and_then(|fields| fields.get(key))
        .map(value_to_form_string)
        .unwrap_or_default()
}

fn update_1601c_period(set_form_input_text: WriteSignal<String>, part: &str, value: String) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            let normalized = if part == "month" { format!("{:0>2}", value.trim()) } else { value.trim().to_string() };
            let field_key = if part == "month" { "txtMonth" } else { "txtYear" };
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                fields.insert(field_key.to_string(), Value::String(normalized.clone()));
            }
            if let Some(period) = root
                .get_mut("return")
                .and_then(Value::as_object_mut)
                .and_then(|ret| ret.get_mut("period"))
                .and_then(Value::as_object_mut)
            {
                if let Ok(number) = normalized.parse::<i64>() {
                    period.insert(part.to_string(), Value::Number(number.into()));
                }
            }
            *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
        }
    });
}

fn update_1601c_tin(set_form_input_text: WriteSignal<String>, key: &str, value: String) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                fields.insert(key.to_string(), Value::String(value));
                let tin1 = fields.get("txtTIN1").map(value_to_form_string).unwrap_or_default();
                let tin2 = fields.get("txtTIN2").map(value_to_form_string).unwrap_or_default();
                let tin3 = fields.get("txtTIN3").map(value_to_form_string).unwrap_or_default();
                let branch = fields.get("txtBranchCode").map(value_to_form_string).unwrap_or_default();
                fields.insert("txtPg2TIN1".to_string(), Value::String(tin1.clone()));
                fields.insert("txtPg2TIN2".to_string(), Value::String(tin2.clone()));
                fields.insert("txtPg2TIN3".to_string(), Value::String(tin3.clone()));
                fields.insert("txtPg2BranchCode".to_string(), Value::String(branch.clone()));
                if let Some(profile) = root.get_mut("profile").and_then(Value::as_object_mut) {
                    profile.insert("tin".to_string(), Value::String(format!("{tin1}-{tin2}-{tin3}-{branch}")));
                }
                *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
            }
        }
    });
}

fn update_checkbox_pair(set_form_input_text: WriteSignal<String>, yes_key: &str, no_key: &str, yes_selected: bool) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                fields.insert(yes_key.to_string(), Value::String(yes_selected.to_string()));
                fields.insert(no_key.to_string(), Value::String((!yes_selected).to_string()));
            }
            if yes_key == "AmendedRtn_1" {
                if let Some(ret) = root.get_mut("return").and_then(Value::as_object_mut) {
                    ret.insert("is_amended".to_string(), Value::Bool(yes_selected));
                }
            }
            *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
        }
    });
}

#[component]
fn PackageDetails(package_preview: ReadSignal<Option<PackagePreviewResponse>>) -> impl IntoView {
    view! {
        <div class="checklist-card">
            <h3>"Package details"</h3>
            {move || match package_preview.get() {
                Some(package) => {
                    let manifest = package.manifest;
                    view! {
                        <dl class="details compact">
                            <dt>"Filename"</dt><dd>{manifest.filename}</dd>
                            <dt>"Remote path"</dt><dd>{manifest.remote_path}</dd>
                            <dt>"Period"</dt><dd>{manifest.period_mm_yyyy}</dd>
                            <dt>"Payload size"</dt><dd>{format!("{} bytes", manifest.payload_size)}</dd>
                            <dt>"Encrypted payload SHA-256"</dt><dd><code>{package.payload_sha256_short}</code><br/><span class="muted hash-full">{manifest.payload_sha256}</span></dd>
                            <dt>"Payload path"</dt><dd>{package.payload_path}</dd>
                        </dl>
                    }.into_view()
                }
                None => view! { <p class="muted">"Validate to render plaintext XML and encrypt/package the payload."</p> }.into_view(),
            }}
        </div>
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
            <p>"Dry-run remains the default. Live submission is still gated by validation, final-copy confirmation, and receipt matching."</p>
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

async fn invoke_json(command: &str, args: Value) -> Result<Value, String> {
    let args = serde_wasm_bindgen::to_value(&args).unwrap_or(JsValue::NULL);
    let value = invoke(command, args)
        .await
        .map_err(|err| err.as_string().unwrap_or_else(|| format!("{err:?}")))?;
    serde_wasm_bindgen::from_value(value).map_err(|err| err.to_string())
}

async fn refresh_jobs_and_submissions(
    set_jobs: WriteSignal<Vec<JobResponse>>,
    set_submissions: WriteSignal<Vec<SafeSubmissionRecordResponse>>,
    set_status: WriteSignal<String>,
) {
    match invoke_json("list_jobs", json!({})).await {
        Ok(value) => match serde_json::from_value::<Vec<JobResponse>>(value) {
            Ok(items) => set_jobs.set(items),
            Err(err) => set_status.set(format!("jobs parse failed: {err}")),
        },
        Err(msg) => set_status.set(format!("list_jobs failed: {msg}")),
    }
    match invoke_json("list_submissions", json!({})).await {
        Ok(value) => match serde_json::from_value::<Vec<SafeSubmissionRecordResponse>>(value) {
            Ok(items) => {
                set_submissions.set(items);
                set_status.set("Tax form flow updated.".to_string());
            }
            Err(err) => set_status.set(format!("submissions parse failed: {err}")),
        },
        Err(msg) => set_status.set(format!("list_submissions failed: {msg}")),
    }
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
    value
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .take(4)
        .collect()
}

fn event_target_checked(ev: &web_sys::Event) -> bool {
    event_target::<web_sys::HtmlInputElement>(ev).checked()
}

fn form_option(code: &str) -> Option<TaxFormOption> {
    TAX_FORMS.iter().copied().find(|option| option.code == code)
}

fn render_jobs(jobs: Vec<JobResponse>) -> View {
    if jobs.is_empty() {
        return view! { <p class="muted">"No queued jobs yet."</p> }.into_view();
    }
    jobs.into_iter().map(|job| {
        view! {
            <article class="record-card">
                <div class="record-header"><strong>{format!("Job #{}", job.id)}</strong><span class="badge info">{job.status}</span></div>
                <dl class="details compact">
                    <dt>"Form"</dt><dd>{job.form_code}</dd>
                    <dt>"Mode"</dt><dd>{job.mode}</dd>
                    <dt>"Attempts"</dt><dd>{format!("{} / {}", job.attempts, job.max_attempts)}</dd>
                    <dt>"Last error"</dt><dd>{job.last_error.unwrap_or_else(|| "—".to_string())}</dd>
                </dl>
            </article>
        }
    }).collect_view().into_view()
}

fn render_submissions(records: Vec<SafeSubmissionRecordResponse>) -> View {
    if records.is_empty() {
        return view! { <p class="muted">"No submissions yet. Validate, queue, and run a dry-run job."</p> }.into_view();
    }
    records.into_iter().map(|record| {
        let status_class = if record.status == "Confirmed" { "badge success" } else { "badge warning" };
        view! {
            <article class="record-card">
                <div class="record-header"><strong>{record.filename.clone()}</strong><span class=status_class>{record.status.clone()}</span></div>
                <dl class="details compact">
                    <dt>"Form"</dt><dd>{record.form_code}</dd>
                    <dt>"Period"</dt><dd>{record.period_mm_yyyy}</dd>
                    <dt>"Remote path"</dt><dd>{record.remote_path}</dd>
                    <dt>"Encrypted payload SHA-256"</dt><dd><code>{record.payload_sha256_short}</code></dd>
                    <dt>"Receipt status"</dt><dd>{record.receipt_status.unwrap_or_else(|| "—".to_string())}</dd>
                </dl>
            </article>
        }
    }).collect_view().into_view()
}

fn latest_submission_filename(records: &[SafeSubmissionRecordResponse]) -> Option<String> {
    records
        .iter()
        .max_by_key(|record| record.updated_unix_seconds)
        .map(|record| record.filename.clone())
}

fn sample_bir_receipt_for_filename(filename: &str) -> String {
    format!(
        "SUBJECT: \"Tax Return Receipt Confirmation\"\nFROM: ebirforms-noreply@bir.gov.ph\nThis confirms receipt of your submission with the following details subject to validation by BIR:\nFile name: {filename}\nDate received by BIR: 15 April 2026\nTime received by BIR: 03:10 PM\nThis is a system-generated email. Please do not reply.\nBureau of Internal Revenue"
    )
}
