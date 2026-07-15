#![recursion_limit = "512"]

mod form_1701q;

use form_1701q::{
    aggregate_amount_payable, calculate_column, decode_bir_text, encode_bir_text, format_centavos,
    is_calculated_box, parse_money_to_centavos, DeductionMethod, MAX_BOX_NUMBER,
};
use leptos::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
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
const FORM_1701Q: &str = include_str!("../../../../tests/fixtures/1701Q/input.json");

#[derive(Clone, Copy, Debug)]
struct TaxFormOption {
    code: &'static str,
    name: &'static str,
    frequency: &'static str,
    sample_input: &'static str,
}

const TAX_FORMS: &[TaxFormOption] = &[
    TaxFormOption {
        code: "1601C",
        name: "Monthly Remittance Return of Income Taxes Withheld on Compensation",
        frequency: "Monthly",
        sample_input: FORM_1601C,
    },
    TaxFormOption {
        code: "2000",
        name: "Documentary Stamp Tax Declaration/Return",
        frequency: "Monthly",
        sample_input: FORM_2000,
    },
    TaxFormOption {
        code: "2550Q",
        name: "Quarterly Value-Added Tax Return",
        frequency: "Quarterly",
        sample_input: FORM_2550Q,
    },
    TaxFormOption {
        code: "0619E",
        name: "Monthly Remittance Form of Creditable Income Taxes Withheld (Expanded)",
        frequency: "Monthly",
        sample_input: FORM_0619E,
    },
    TaxFormOption {
        code: "1601EQ",
        name: "Quarterly Remittance Return of Creditable Income Taxes Withheld (Expanded)",
        frequency: "Quarterly",
        sample_input: FORM_1601EQ,
    },
    TaxFormOption {
        code: "1701Q",
        name: "Quarterly Income Tax Return for Individuals, Estates and Trusts",
        frequency: "Quarterly",
        sample_input: FORM_1701Q,
    },
    TaxFormOption {
        code: "1702Q",
        name: "Quarterly Income Tax Return for Corporations",
        frequency: "Quarterly",
        sample_input: FORM_1702Q,
    },
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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ReceiptPollReportResponse {
    scanned: usize,
    confirmed: Vec<SafeSubmissionRecordResponse>,
    errors: Vec<String>,
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (route, set_route) = create_signal("Dashboard".to_string());
    let (previous_route, set_previous_route) = create_signal("Dashboard".to_string());
    let (last_route, set_last_route) = create_signal("Dashboard".to_string());
    let (status, set_status) = create_signal(
        "Ready. Create or choose a profile, then open a form from the Tax Form Library."
            .to_string(),
    );
    let (theme, set_theme) = create_signal("system".to_string());
    let (submission_mode, set_submission_mode) = create_signal("dry_run".to_string());
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
    let (form_input_text, set_form_input_text) =
        create_signal(initial_form.sample_input.to_string());
    let (saved_form_input_text, set_saved_form_input_text) =
        create_signal(initial_form.sample_input.to_string());
    let (form_locked, set_form_locked) = create_signal(false);
    let (plaintext_preview, set_plaintext_preview) =
        create_signal("Validate a form to preview the plaintext XML.".to_string());
    let (package_preview, set_package_preview) = create_signal(None::<PackagePreviewResponse>);
    let (jobs, set_jobs) = create_signal(Vec::<JobResponse>::new());
    let (submissions, set_submissions) = create_signal(Vec::<SafeSubmissionRecordResponse>::new());
    let (receipt_text, set_receipt_text) = create_signal(sample_bir_receipt_for_filename(
        "12345678900000-1601C-062026#authorized@example.test#.xml",
    ));
    let (final_copy_confirmed, set_final_copy_confirmed) = create_signal(false);
    let (waiting_for_receipt, set_waiting_for_receipt) = create_signal(false);

    create_effect(move |_| {
        let current = route.get();
        let last = last_route.get_untracked();
        if current != last {
            set_previous_route.set(last);
            set_last_route.set(current);
        }
    });

    if let Some(window) = web_sys::window() {
        let keydown = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |event: web_sys::KeyboardEvent| {
                if (event.meta_key() || event.ctrl_key()) && event.key() == "ArrowLeft" {
                    event.prevent_default();
                    let destination = previous_route.get_untracked();
                    if route.get_untracked() != destination {
                        set_route.set(destination.clone());
                        set_status.set(format!("Returned to {destination}."));
                    }
                }
            },
        );
        let _ =
            window.add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref());
        on_cleanup(move || {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback(
                    "keydown",
                    keydown.as_ref().unchecked_ref(),
                );
            }
        });
    }

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
                if let Some(saved_mode) = snapshot
                    .get("settings")
                    .and_then(|settings| settings.get("submission_mode"))
                    .and_then(|mode| mode.as_str())
                    .and_then(normalize_submission_mode)
                {
                    set_submission_mode.set(saved_mode.to_string());
                }
                let loaded_profiles: Vec<TaxpayerProfileResponse> = serde_json::from_value(
                    snapshot
                        .get("profiles")
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                )
                .unwrap_or_default();
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
        profiles
            .get()
            .into_iter()
            .find(|p| Some(p.profile_id.clone()) == id)
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
                    set_status.set(format!(
                        "update_settings failed; theme preference reverted: {msg}"
                    ));
                }
            }
        });
    };

    let set_submission_mode_preference = move |mode_name: &'static str| {
        let Some(next_mode) = normalize_submission_mode(mode_name).map(str::to_string) else {
            set_status.set(format!("Invalid submission mode: {mode_name}"));
            return;
        };
        if next_mode == "live" {
            let confirmed = web_sys::window()
                .and_then(|window| window.confirm_with_message(
                    "Switch to LIVE submission mode? Future Submit Final Copy actions will upload encrypted payloads to the BIR filing endpoint instead of dry-run only. Continue only if this filing is authorized and ready."
                ).ok())
                .unwrap_or(false);
            if !confirmed {
                set_status.set("Live submission mode was not enabled.".to_string());
                return;
            }
        }
        let previous_mode = submission_mode.get_untracked();
        set_submission_mode.set(next_mode.clone());
        set_status.set(format!("Saving {next_mode} submission mode…"));
        spawn_local(async move {
            match invoke_json("update_submission_mode", json!({"mode": next_mode})).await {
                Ok(value) => {
                    let saved_mode = value
                        .get("submission_mode")
                        .and_then(|mode| mode.as_str())
                        .and_then(normalize_submission_mode)
                        .unwrap_or("dry_run");
                    set_submission_mode.set(saved_mode.to_string());
                    set_status.set(format!("Submission mode saved: {saved_mode}"));
                }
                Err(msg) => {
                    set_submission_mode.set(previous_mode);
                    set_status.set(format!(
                        "update_submission_mode failed; submission mode reverted: {msg}"
                    ));
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
                    <h1>"PH Tax Forms"</h1>
                    <p class="muted">"Unofficial synthetic filing demo"</p>
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
                    "Settings" => view! { <Settings theme=theme set_theme_preference=set_theme_preference submission_mode=submission_mode set_submission_mode_preference=set_submission_mode_preference pin=settings_pin set_pin=set_settings_pin lock_now=lock_now /> }.into_view(),
                    "TaxFormFlow" => view! { <TaxFormFlow active_profile_id=active_profile_id set_route=set_route selected_form=selected_form form_input_text=form_input_text set_form_input_text=set_form_input_text saved_form_input_text=saved_form_input_text set_saved_form_input_text=set_saved_form_input_text form_locked=form_locked set_form_locked=set_form_locked plaintext_preview=plaintext_preview set_plaintext_preview=set_plaintext_preview package_preview=package_preview set_package_preview=set_package_preview jobs=jobs set_jobs=set_jobs submissions=submissions set_submissions=set_submissions receipt_text=receipt_text set_receipt_text=set_receipt_text submission_mode=submission_mode final_copy_confirmed=final_copy_confirmed set_final_copy_confirmed=set_final_copy_confirmed waiting_for_receipt=waiting_for_receipt set_waiting_for_receipt=set_waiting_for_receipt set_status=set_status /> }.into_view(),
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
            let active_id = active_profile_id.get_untracked();
            let active_profile = profiles
                .get_untracked()
                .into_iter()
                .find(|profile| Some(profile.profile_id.clone()) == active_id);
            let prepared_input = if option.code == "1701Q" {
                active_profile
                    .as_ref()
                    .map(|profile| personalize_form_input(option.code, option.sample_input, profile))
                    .unwrap_or_else(|| option.sample_input.to_string())
            } else {
                option.sample_input.to_string()
            };
            set_selected_form.set(option.code.to_string());
            set_form_input_text.set(prepared_input.clone());
            set_saved_form_input_text.set(prepared_input);
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
    submission_mode: ReadSignal<String>,
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
        set_status
            .set("Form reopened for editing; final-copy confirmation was cleared.".to_string());
    };

    let validate_form = move || {
        if active_profile_id.get_untracked().is_none() {
            set_status.set(
                "Create and save a taxpayer profile before validating a tax form.".to_string(),
            );
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
                        set_receipt_text
                            .set(sample_bir_receipt_for_filename(&package.manifest.filename));
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
            set_status
                .set("Create and save a taxpayer profile before queueing a tax form.".to_string());
            return;
        }
        let form_code = selected_form.get_untracked();
        let Ok(input_json) = serde_json::from_str::<Value>(&saved_form_input_text.get_untracked())
        else {
            set_status.set("Queue failed: saved form JSON is invalid.".to_string());
            return;
        };
        let mode = submission_mode.get_untracked();
        set_status.set(format!("Queueing {mode} job…"));
        spawn_local(async move {
            match invoke_json(
                "queue_tax_form",
                json!({"formCode": form_code, "input": input_json, "mode": mode}),
            )
            .await
            {
                Ok(_) => refresh_jobs_and_submissions(set_jobs, set_submissions, set_status).await,
                Err(msg) => set_status.set(format!("queue_tax_form failed: {msg}")),
            }
        });
    };

    let run_queue = move || {
        let mode = submission_mode.get_untracked();
        set_status.set(format!("Running {mode} queue…"));
        spawn_local(async move {
            match invoke_json("run_queue", json!({"mode": mode, "limit": 10})).await {
                Ok(_) => refresh_jobs_and_submissions(set_jobs, set_submissions, set_status).await,
                Err(msg) => set_status.set(format!("run_queue failed: {msg}")),
            }
        });
    };

    let simulate_receipt = move || {
        let filename = package_preview
            .get_untracked()
            .map(|p| p.manifest.filename)
            .or_else(|| latest_submission_filename(&submissions.get_untracked()));
        let Some(filename) = filename else {
            set_status
                .set("Run Validate or the dry-run queue before simulating a receipt.".to_string());
            return;
        };
        let synthetic_receipt = sample_bir_receipt_for_filename(&filename);
        set_receipt_text.set(synthetic_receipt.clone());
        set_status.set("Matching synthetic BIR receipt…".to_string());
        spawn_local(async move {
            match invoke_json("match_receipt", json!({"receiptText": synthetic_receipt})).await {
                Ok(value) => {
                    match serde_json::from_value::<Vec<SafeSubmissionRecordResponse>>(value) {
                        Ok(records) => {
                            set_submissions.set(records);
                            set_waiting_for_receipt.set(false);
                            set_status
                                .set("Receipt matched against submission records.".to_string());
                        }
                        Err(err) => set_status.set(format!("receipt response parse failed: {err}")),
                    }
                }
                Err(msg) => set_status.set(format!("match_receipt failed: {msg}")),
            }
        });
    };

    let poll_himalaya = move || {
        set_status.set("Checking receipt mailbox with Himalaya…".to_string());
        spawn_local(async move {
            match invoke_json(
                "poll_himalaya_receipts",
                json!({"account": null, "folder": "INBOX", "query": ["subject", "Tax Return Receipt Confirmation"], "limit": 25}),
            )
            .await
            {
                Ok(value) => match serde_json::from_value::<ReceiptPollReportResponse>(value) {
                    Ok(report) => {
                        refresh_jobs_and_submissions(set_jobs, set_submissions, set_status).await;
                        if report.errors.is_empty() {
                            set_status.set(format!(
                                "Himalaya scanned {} receipt email(s); confirmed {} submission(s).",
                                report.scanned,
                                report.confirmed.len()
                            ));
                        } else {
                            set_status.set(format!(
                                "Himalaya scanned {} email(s), confirmed {}, with {} parse/error item(s).",
                                report.scanned,
                                report.confirmed.len(),
                                report.errors.len()
                            ));
                        }
                    }
                    Err(err) => set_status.set(format!("Himalaya poll response parse failed: {err}")),
                },
                Err(msg) => set_status.set(format!("poll_himalaya_receipts failed: {msg}")),
            }
        });
    };

    let submit_final_copy = move || {
        if active_profile_id.get_untracked().is_none() {
            set_status.set(
                "Create and save a taxpayer profile before submitting a final copy.".to_string(),
            );
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
        let Ok(input_json) = serde_json::from_str::<Value>(&saved_form_input_text.get_untracked())
        else {
            set_status.set("Submit Final Copy failed: validated form JSON is invalid.".to_string());
            return;
        };
        let mode = submission_mode.get_untracked();
        set_status.set(format!("Submit Final Copy: queueing and running {mode} delivery…"));
        spawn_local(async move {
            match invoke_json(
                "queue_tax_form",
                json!({"formCode": form_code, "input": input_json, "mode": mode.clone()}),
            )
            .await
            {
                Ok(_) => {}
                Err(msg) => {
                    set_status.set(format!("Submit Final Copy queue failed: {msg}"));
                    return;
                }
            }
            match invoke_json("run_queue", json!({"mode": mode, "limit": 10})).await {
                Ok(_) => {
                    set_waiting_for_receipt.set(true);
                    refresh_jobs_and_submissions(set_jobs, set_submissions, set_status).await;
                    set_status.set(
                        "Submit Final Copy queued and ran. Waiting for a BIR receipt confirmation."
                            .to_string(),
                    );
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
                    <button on:click=move |_| queue_dry_run() disabled=move || !form_locked.get() || waiting_for_receipt.get()>{move || if submission_mode.get() == "live" { "Queue live" } else { "Queue dry-run" }}</button>
                    <button on:click=move |_| run_queue() disabled=move || waiting_for_receipt.get()>{move || if submission_mode.get() == "live" { "Run live queue" } else { "Run dry-run queue" }}</button>
                    <button on:click=move |_| simulate_receipt() disabled=move || package_preview.get().is_none() && submissions.get().is_empty()>"Simulate received BIR receipt"</button>
                    <button on:click=move |_| poll_himalaya() disabled=move || submissions.get().is_empty()>"Check receipt mailbox (Himalaya)"</button>
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
    if form_code == "1701Q" {
        return render_1701q_physical_form(input, set_form_input_text, locked);
    }

    render_pdf_physical_form(form_code, input, set_form_input_text, locked)
}

fn render_pdf_physical_form(
    form_code: String,
    input: Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let mut field_items: Vec<(String, String)> = input
        .get("fields")
        .and_then(Value::as_object)
        .map(|fields| {
            fields
                .iter()
                .map(|(key, value)| (key.clone(), value_to_form_string(value)))
                .collect()
        })
        .unwrap_or_default();
    field_items.sort_by(|a, b| {
        physical_field_sort_key(&form_code, &a.0).cmp(&physical_field_sort_key(&form_code, &b.0))
    });

    let header_fields =
        fields_for_physical_section(&form_code, &field_items, PhysicalSectionKind::Header);
    let background_fields =
        fields_for_physical_section(&form_code, &field_items, PhysicalSectionKind::Background);
    let computation_fields =
        fields_for_physical_section(&form_code, &field_items, PhysicalSectionKind::Computation);
    let vat_fields = fields_for_physical_section(
        &form_code,
        &field_items,
        PhysicalSectionKind::VatComputation,
    );
    let payment_fields =
        fields_for_physical_section(&form_code, &field_items, PhysicalSectionKind::Payment);
    let schedule_fields =
        fields_for_physical_section(&form_code, &field_items, PhysicalSectionKind::Schedule);

    view! {
        <div class=format!("bir-paper form-{}", form_code.to_ascii_lowercase())>
            <div class="bir-title-grid multi-form-title">
                <div class="bir-form-no"><span>"BIR Form No."</span><strong>{form_code.clone()}</strong><small>{physical_form_version(&form_code)}</small></div>
                <div class="bir-title"><strong>{physical_form_title(&form_code)}</strong><span>{physical_form_subtitle(&form_code)}</span></div>
                <div class="bir-barcode">{format!("{} PDF-LIKE DATA ENTRY", form_code)}</div>
            </div>
            <div class="bir-grid physical-top-strip">
                {render_pdf_header_controls(&form_code, &input, header_fields, locked, set_form_input_text)}
            </div>
            <div class="bir-section-title">"Part I – Background Information"</div>
            <div class="bir-grid physical-background-grid">
                {render_physical_boxes(&form_code, background_fields, locked, set_form_input_text)}
            </div>
            <div class="bir-section-title">{physical_computation_title(&form_code)}</div>
            <div class="bir-table computation-table">
                {render_physical_rows(&form_code, computation_fields, locked, set_form_input_text)}
            </div>
            {if !vat_fields.is_empty() {
                view! {
                    <div class="bir-section-title">"Part IV – Computation of VAT Payable / Excess Input Tax"</div>
                    <div class="bir-table computation-table">{render_physical_rows(&form_code, vat_fields, locked, set_form_input_text)}</div>
                }.into_view()
            } else { view! {}.into_view() }}
            <div class="bir-section-title">{physical_payment_title(&form_code)}</div>
            <div class="bir-payment-grid physical-payment-grid">
                {render_physical_payment_rows(&form_code, payment_fields, locked, set_form_input_text)}
            </div>
            {if !schedule_fields.is_empty() {
                view! {
                    <div class="bir-section-title">{physical_schedule_title(&form_code)}</div>
                    <div class="bir-grid physical-schedule-grid">{render_physical_boxes(&form_code, schedule_fields, locked, set_form_input_text)}</div>
                }.into_view()
            } else { view! {}.into_view() }}
        </div>
    }.into_view()
}


fn render_1701q_physical_form(
    input: Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let taxpayer_estate_or_trust = field_bool(&input, "ui1701q:taxpayer_type_estate")
        || field_bool(&input, "ui1701q:taxpayer_type_trust");
    let taxpayer_8_percent = field_bool(&input, "ui1701q:taxpayer_atc_ii015")
        || field_bool(&input, "ui1701q:taxpayer_atc_ii017")
        || field_bool(&input, "ui1701q:taxpayer_atc_ii016");
    let taxpayer_graduated = !taxpayer_8_percent;
    let spouse_8_percent = field_bool(&input, "ui1701q:spouse_atc_ii015")
        || field_bool(&input, "ui1701q:spouse_atc_ii017")
        || field_bool(&input, "ui1701q:spouse_atc_ii016");
    let taxpayer_osd = field_bool(&input, "frm1701:optMethodOfDeduction23:_2");
    let spouse_osd = field_bool(&input, "frm1701:optMethodOfDeduction24:_2");
    let page2_tin = format!("{}-{}-{}-{}", field_value(&input, "frm1701q:txt5TIN1"), field_value(&input, "frm1701q:txt5TIN2"), field_value(&input, "frm1701q:txt5TIN3"), field_value(&input, "frm1701q:txt5BranchCode"));
    let page2_name = decode_bir_text(&field_value(&input, "frm1701q:txtTaxPayername"));

    view! {
        <div class="form-1701q-pages">
        <div class="bir-paper form-1701q bir-page page-one">
            <div class="bir-title-grid multi-form-title">
                <div class="bir-form-no"><span>"BIR Form No."</span><strong>"1701Q"</strong><small>"January 2018 (ENCS)"</small></div>
                <div class="bir-title"><strong>"Quarterly Income Tax Return"</strong><span>"For Individuals, Estates and Trusts"</span></div>
                <div class="bir-barcode">"1701Q 01/18ENCS P1"</div>
            </div>
            <div class="bir-grid physical-top-strip">
                {render_1701q_digits_box("1", "For the Year", "frm1701q:txtYear", 4, &input, set_form_input_text, locked)}
                {render_1701q_choice_box("2", "Quarter", vec![("First", "frm1701q:DateQuarter_1"), ("Second", "frm1701q:DateQuarter_2"), ("Third", "frm1701q:DateQuarter_3")], &input, set_form_input_text, locked, None)}
                {render_1701q_pair_box("3", "Amended Return?", "frm1701q:AmendedRtn_1", "frm1701q:AmendedRtn_2", &input, set_form_input_text, locked)}
                {render_1701q_digits_box("4", "Number of Sheet/s Attached", "frm1701q:txtSheets", 2, &input, set_form_input_text, locked)}
            </div>

            <div class="bir-section-title">"Part I – Background Information on Taxpayer/Filer"</div>
            <div class="bir-grid physical-background-grid">
                {render_1701q_readonly_box("5", "Taxpayer Identification Number (TIN)", "frm1701q:txt5TIN1", "frm1701q:txt5TIN2", "frm1701q:txt5TIN3", "frm1701q:txt5BranchCode", &input)}
                {render_1701q_readonly_single("6", "RDO Code", "frm1701q:txt5RDOCode", &input)}
                {render_1701q_choice_box("7", "Taxpayer/Filer Type", vec![("Single Proprietor", "ui1701q:taxpayer_type_single"), ("Professional", "ui1701q:taxpayer_type_professional"), ("Estate", "ui1701q:taxpayer_type_estate"), ("Trust", "ui1701q:taxpayer_type_trust")], &input, set_form_input_text, locked, Some("taxpayer_type"))}
                {render_1701q_atc_box("8", false, taxpayer_estate_or_trust, &input, set_form_input_text, locked)}
                {render_1701q_readonly_single("9", "Taxpayer/Filer's Name", "frm1701q:txtTaxPayername", &input)}
                {render_1701q_text_box("10", "Registered Address", "frm1701q:txt11Address", &input, set_form_input_text, locked, false)}
                {render_1701q_readonly_single("10A", "Zip Code", "frm1701q:txt14zip", &input)}
                {render_1701q_date_box("11", "Date of Birth (MM/DD/YYYY)", "frm1701q:txt13BirthMonth", "frm1701q:txt13BirthDay", "frm1701q:txt13BirthYear", &input, set_form_input_text, locked)}
                {render_1701q_readonly_single("12", "Email Address", "txtEmail", &input)}
                {render_1701q_text_box("13", "Citizenship", "ui1701q:taxpayer_citizenship", &input, set_form_input_text, locked, false)}
                {render_1701q_text_box("14", "Foreign Tax Number (if applicable)", "ui1701q:taxpayer_foreign_tax_number", &input, set_form_input_text, locked, false)}
                {render_1701q_pair_box("15", "Claiming Foreign Tax Credits?", "frm1701q:SelTreaty_1", "frm1701q:SelTreaty_2", &input, set_form_input_text, locked)}
                {render_1701q_tax_rate_box("16", false, taxpayer_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_method_box("16A", "Method of Deduction", "frm1701:optMethodOfDeduction23:_1", "frm1701:optMethodOfDeduction23:_2", taxpayer_8_percent, &input, set_form_input_text, locked)}
            </div>

            <div class="bir-section-title">"Part II – Background Information on Spouse (if applicable)"</div>
            <div class="bir-grid physical-background-grid">
                {render_1701q_editable_tin_box("17", "Spouse's TIN", "frm1701q:txt7TIN1", "frm1701q:txt7TIN2", "frm1701q:txt7TIN3", "frm1701q:txt7BranchCode", &input, set_form_input_text, locked)}
                {render_1701q_text_box("18", "RDO Code", "frm1701q:txt7RDOCode", &input, set_form_input_text, locked, false)}
                {render_1701q_choice_box("19", "Filer's Spouse Type", vec![("Single Proprietor", "ui1701q:spouse_type_single"), ("Professional", "ui1701q:spouse_type_professional"), ("Compensation Earner", "ui1701q:spouse_type_compensation")], &input, set_form_input_text, locked, None)}
                {render_1701q_atc_box("20", true, false, &input, set_form_input_text, locked)}
                {render_1701q_text_box("21", "Spouse's Name", "frm1701q:txtSpousename", &input, set_form_input_text, locked, false)}
                {render_1701q_text_box("22", "Citizenship", "ui1701q:spouse_citizenship", &input, set_form_input_text, locked, false)}
                {render_1701q_text_box("23", "Foreign Tax Number, if applicable", "ui1701q:spouse_foreign_tax_number", &input, set_form_input_text, true, false)}
                {render_1701q_choice_box("24", "Claiming Foreign Tax Credits?", vec![("Yes", "ui1701q:spouse_foreign_tax_credits_yes"), ("No", "ui1701q:spouse_foreign_tax_credits_no")], &input, set_form_input_text, locked, None)}
                {render_1701q_tax_rate_box("25", true, spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_method_box("25A", "Method of Deduction", "frm1701:optMethodOfDeduction24:_1", "frm1701:optMethodOfDeduction24:_2", spouse_8_percent, &input, set_form_input_text, locked)}
            </div>

            <div class="bir-section-title">"Part III – Total Tax Payable"</div>
            <div class="bir-table computation-table">
                <div class="bir-two-col-header"><span>"Particulars"</span><strong>"A) Taxpayer/Filer"</strong><strong>"B) Spouse"</strong></div>
                {render_1701q_two_amount_row("26", "Tax Due (Form Part V, Schedule I-Item 46 OR Schedule II-Item 54)", "frm1701q:txt26A", "frm1701q:txt26B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("27", "Less: Tax Credits/Payments (From Part V, Schedule III-Item 62)", "frm1701q:txt27A", "frm1701q:txt27B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("28", "Tax Payable/(Overpayment) (Item 26 Less Item 27)(From Part V, Item 63)", "frm1701q:txt28A", "frm1701q:txt28B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("29", "Add: Total Penalties (From Part V, Schedule IV-Item 67)", "frm1701q:txt29A", "frm1701q:txt29B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("30", "Total Amount Payable/(Overpayment) (Sum of Items 28 and 29)(From Part V, Item 68)", "frm1701q:txt30A", "frm1701q:txt30B", &input, set_form_input_text, locked)}
                {render_1701q_aggregate_amount_row("31", "Aggregate Amount Payable/(Overpayment) (Sum of Items 30A and 30B)", "frm1701q:txt31A", &input, set_form_input_text, locked)}
            </div>

            <div class="bir-section-title">"Part IV – Details of Payment"</div>
            <div class="payment-table-1701q" aria-label="Details of Payment - non-fillable">
                <div class="payment-header"><strong>"Particulars"</strong><strong>"Drawee Bank/Agency"</strong><strong>"Number"</strong><strong>"Date (MM/DD/YYYY)"</strong><strong>"Amount"</strong></div>
                {render_1701q_disabled_payment_row("32", "Cash/Bank Debit Memo")}
                {render_1701q_disabled_payment_row("33", "Check")}
                {render_1701q_disabled_payment_row("34", "Tax Debit Memo")}
                {render_1701q_disabled_payment_row("35", "Others (specify)")}
            </div>
        </div>

        <div class="bir-paper form-1701q bir-page page-two">
            <div class="bir-title-grid multi-form-title page-two-heading">
                <div class="bir-form-no"><span>"BIR Form No."</span><strong>"1701Q"</strong><small>"Page 2"</small></div>
                <div class="bir-title"><strong>"Part V – Computation of Tax Due"</strong><span>"If graduated rate, fill in items 36 to 46; if 8%, fill in items 47 to 54"</span></div>
                <div class="bir-barcode">"1701Q 01/18ENCS P2"</div>
            </div>
            <div class="page-two-identity-strip">
                <label><strong>"TIN"</strong><input prop:value=page2_tin readonly /></label>
                <label><strong>"Taxpayer/Filer's Last Name"</strong><input prop:value=page2_name readonly /></label>
            </div>
            <div class="bir-section-title">"Part V – Computation of Tax Due"</div>
            <div class="bir-two-col-header schedule-columns"><span>"Declaration this Quarter"</span><strong>"A) Taxpayer/Filer"</strong><strong>"B) Spouse"</strong></div>
            <div class="schedule-instruction">"If graduated rate, fill in items 36 to 46; if 8%, fill in items 47 to 54"</div>
            <div class="bir-table computation-table">
                <div class="bir-section-title inline">"Schedule I – For Graduated IT Rate"</div>
                {render_1701q_two_amount_row_disabled("36", "Sales/Revenues/Receipts/Fees (net of sales returns, allowances, and discounts)", "frm1701q:txt36A", "frm1701q:txt36B", taxpayer_8_percent, spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("37", "Less: Cost of Sales/Services (applicable only if availing Itemized Deductions)", "frm1701q:txt37A", "frm1701q:txt37B", taxpayer_8_percent || taxpayer_osd, spouse_8_percent || spouse_osd, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("38", "Gross Income/(Loss) from Operation (Item 36 Less Item 37)", "frm1701q:txt38A", "frm1701q:txt38B", taxpayer_8_percent, spouse_8_percent, &input, set_form_input_text, locked)}
                <div class="bir-subrow">"Less: Allowable Deductions"</div>
                {render_1701q_two_amount_row_disabled("39", "Total Allowable Itemized Deductions", "frm1701q:txt38C", "frm1701q:txt38D", taxpayer_8_percent || taxpayer_osd, spouse_8_percent || spouse_osd, &input, set_form_input_text, locked)}
                <div class="bir-subrow centered">"OR"</div>
                {render_1701q_two_amount_row_disabled("40", "Optional Standard Deduction (OSD) (40% of Item 36)", "frm1701q:txt38E", "frm1701q:txt38F", taxpayer_8_percent, spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("41", "Net Income/(Loss) This Quarter (Item 38 Less Either Item 39 OR 40)", "frm1701q:txt38G", "frm1701q:txt38H", taxpayer_8_percent, spouse_8_percent, &input, set_form_input_text, locked)}
                <div class="bir-subrow">"Add:"</div>
                {render_1701q_two_amount_row_disabled("42", "Taxable Income/(Loss) Previous Quarter/s", "frm1701q:txt38I", "frm1701q:txt38J", taxpayer_8_percent, spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_specify_amount_row("43", "Non-Operating Income", "ui1701q:txt43Specify", "frm1701q:txt38K", "frm1701q:txt38L", taxpayer_8_percent, spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("44", "Amount Received/Share in Income by a Partner from General Professional Partnership (GPP)", "frm1701q:txt38M", "frm1701q:txt38N", taxpayer_8_percent, spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("45", "Total Taxable Income/(Loss) To Date (Sum of Items 41 to 44)", "frm1701q:txt39A", "frm1701q:txt39B", taxpayer_8_percent, spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("46", "Tax Due (Item 45 x Applicable Tax Rate based on Tax Table below)(To Part III, Item 26)", "ui1701q:txt46A", "ui1701q:txt46B", taxpayer_8_percent, spouse_8_percent, &input, set_form_input_text, locked)}

                <div class="bir-section-title inline">"Schedule II – For 8% IT Rate"</div>
                {render_1701q_two_amount_row_disabled("47", "Sales/Revenues/Receipts/Fees (net of sales returns, allowances and discounts)", "frm1701q:txt40A", "frm1701q:txt40B", taxpayer_graduated, !spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_specify_amount_row("48", "Add: Non-Operating Income", "ui1701q:txt48Specify", "frm1701q:txt40C", "frm1701q:txt40D", taxpayer_graduated, !spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("49", "Total Income for the quarter (Sum of Items 47 and 48)", "frm1701q:txt40E", "frm1701q:txt40F", taxpayer_graduated, !spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("50", "Add: Total Taxable Income/(Loss) Previous Quarter (Item 51 of previous quarter)", "frm1701q:txt40G", "frm1701q:txt40H", taxpayer_graduated, !spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("51", "Cumulative Taxable Income/(Loss) as of This Quarter (Sum of Items 49 and 50)", "frm1701q:txt41A", "frm1701q:txt41B", taxpayer_graduated, !spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("52", "Less: Allowable reduction from gross sales/receipts and other non-operating income of purely self-employed individuals and/or professionals in the amount of Php 250,000.00", "ui1701q:txt52A", "ui1701q:txt52B", taxpayer_graduated, !spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("53", "Taxable Income/(Loss) To Date (Items 51 Less Item 52)", "ui1701q:txt53A", "ui1701q:txt53B", taxpayer_graduated, !spouse_8_percent, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row_disabled("54", "Tax Due (Item 53 x 8% Tax Rate)(To Part III, Item 26)", "ui1701q:txt54A", "ui1701q:txt54B", taxpayer_graduated, !spouse_8_percent, &input, set_form_input_text, locked)}

                <div class="bir-section-title inline">"Schedule III – Tax Credits/Payments"</div>
                {render_1701q_two_amount_row("55", "Prior Year's Excess Credits", "ui1701q:txt55A", "ui1701q:txt55B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("56", "Tax Payment/s for the Previous Quarter/s", "ui1701q:txt56A", "ui1701q:txt56B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("57", "Creditable Tax Withheld for the Previous Quarter/s", "ui1701q:txt57A", "ui1701q:txt57B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("58", "Creditable Tax Withheld per BIR Form No. 2307 for this Quarter", "ui1701q:txt58A", "ui1701q:txt58B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("59", "Tax Paid in Return Previously Filed, if this is an Amended Return", "ui1701q:txt59A", "ui1701q:txt59B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("60", "Foreign Tax Credits, if applicable", "ui1701q:txt60A", "ui1701q:txt60B", &input, set_form_input_text, locked)}
                {render_1701q_specify_amount_row("61", "Other Tax Credits/Payments", "ui1701q:txt61Specify", "ui1701q:txt61A", "ui1701q:txt61B", false, false, &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("62", "Total Tax Credits/Payments (Sum of Items 55 to 61)(To Part III, Item 27)", "ui1701q:txt62A", "ui1701q:txt62B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("63", "Tax Payable/(Overpayment)(Item 46 or 54, Less Item 62)(To Part III, Item 28)", "ui1701q:txt63A", "ui1701q:txt63B", &input, set_form_input_text, locked)}

                <div class="bir-section-title inline">"Schedule IV – Penalties"</div>
                {render_1701q_two_amount_row("64", "Surcharge", "ui1701q:txt64A", "ui1701q:txt64B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("65", "Interest", "ui1701q:txt65A", "ui1701q:txt65B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("66", "Compromise", "ui1701q:txt66A", "ui1701q:txt66B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("67", "Total Penalties (Sum of Items 64 to 66)(To Part III, Item 29)", "ui1701q:txt67A", "ui1701q:txt67B", &input, set_form_input_text, locked)}
                {render_1701q_two_amount_row("68", "Total Amount Payable/(Overpayment) (Sum of Items 63 and 67)(To Part III, Item 30)", "ui1701q:txt68A", "ui1701q:txt68B", &input, set_form_input_text, locked)}
            </div>
            {render_1701q_tax_rate_tables()}
        </div>
        </div>
    }.into_view()
}

fn render_1701q_digits_box(
    item: &'static str,
    label: &'static str,
    key: &'static str,
    max_digits: usize,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let value = field_value(input, key);
    view! {
        <label class="bir-box">{format!("{item} {label}")}
            <input data-form-field=key inputmode="numeric" maxlength=max_digits prop:value=value prop:readonly=locked on:input=move |ev| update_field_digits(set_form_input_text, key, event_target_value(&ev), max_digits) />
        </label>
    }.into_view()
}

fn render_1701q_pair_box(
    item: &'static str,
    label: &'static str,
    yes_key: &'static str,
    no_key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let group_name = format!("1701q-{item}");
    let yes_checked = field_bool(input, yes_key);
    let no_checked = field_bool(input, no_key);
    view! {
        <fieldset class="bir-box checkbox-pair radio-group-1701q">
            <legend>{format!("{item} {label}")}</legend>
            <label><input type="radio" name=group_name.clone() prop:checked=yes_checked prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) { update_pair_fields(set_form_input_text, yes_key, no_key, true) } />"Yes"</label>
            <label><input type="radio" name=group_name prop:checked=no_checked prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) { update_pair_fields(set_form_input_text, yes_key, no_key, false) } />"No"</label>
        </fieldset>
    }.into_view()
}

fn is_bir_encoded_1701q_text_field(key: &str) -> bool {
    matches!(
        key,
        "frm1701q:txtTaxPayername"
            | "frm1701q:txtSpousename"
            | "frm1701q:txt11Address"
            | "frm1701q:txt12Address"
    )
}

fn display_1701q_field_value(key: &str, stored_value: String) -> String {
    if is_bir_encoded_1701q_text_field(key) {
        decode_bir_text(&stored_value)
    } else {
        stored_value
    }
}

fn render_1701q_text_box(
    item: &'static str,
    label: &'static str,
    key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
    disabled: bool,
) -> View {
    let encoded_text = is_bir_encoded_1701q_text_field(key);
    let value = display_1701q_field_value(key, field_value(input, key));
    view! {
        <label class="bir-box span-2">{format!("{item} {label}")}
            <input data-form-field=key prop:value=value prop:readonly=locked || disabled on:input=move |ev| {
                let value = event_target_value(&ev);
                update_field_string(set_form_input_text, key, if encoded_text { encode_bir_text(&value) } else { value });
            } />
        </label>
    }.into_view()
}

fn render_1701q_editable_tin_box(
    item: &'static str,
    label: &'static str,
    key1: &'static str,
    key2: &'static str,
    key3: &'static str,
    key4: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let value1 = field_value(input, key1);
    let value2 = field_value(input, key2);
    let value3 = field_value(input, key3);
    let value4 = field_value(input, key4);
    view! {
        <label class="bir-box span-2"><span>{format!("{item} {label}")}</span>
            <div class="split-inputs tin-inputs">
                <input data-form-field=key1 inputmode="numeric" maxlength=3 prop:value=value1 prop:readonly=locked on:input=move |ev| update_field_digits(set_form_input_text, key1, event_target_value(&ev), 3) /><span>"-"</span>
                <input data-form-field=key2 inputmode="numeric" maxlength=3 prop:value=value2 prop:readonly=locked on:input=move |ev| update_field_digits(set_form_input_text, key2, event_target_value(&ev), 3) /><span>"-"</span>
                <input data-form-field=key3 inputmode="numeric" maxlength=3 prop:value=value3 prop:readonly=locked on:input=move |ev| update_field_digits(set_form_input_text, key3, event_target_value(&ev), 3) /><span>"-"</span>
                <input data-form-field=key4 inputmode="numeric" maxlength=5 prop:value=value4 prop:readonly=locked on:input=move |ev| update_field_digits(set_form_input_text, key4, event_target_value(&ev), 5) />
            </div>
        </label>
    }.into_view()
}

fn render_1701q_readonly_single(item: &'static str, label: &'static str, key: &'static str, input: &Value) -> View {
    let value = display_1701q_field_value(key, field_value(input, key));
    view! { <label class="bir-box"><span>{format!("{item} {label}")}</span><input prop:value=value readonly /></label> }.into_view()
}

fn render_1701q_readonly_box(
    item: &'static str,
    label: &'static str,
    key1: &'static str,
    key2: &'static str,
    key3: &'static str,
    branch_key: &'static str,
    input: &Value,
) -> View {
    let value = format!("{}-{}-{}-{}", field_value(input, key1), field_value(input, key2), field_value(input, key3), field_value(input, branch_key));
    view! { <label class="bir-box span-2"><span>{format!("{item} {label}")}</span><input prop:value=value readonly /></label> }.into_view()
}

fn render_1701q_date_box(
    item: &'static str,
    label: &'static str,
    month_key: &'static str,
    day_key: &'static str,
    year_key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let month = field_value(input, month_key);
    let day = field_value(input, day_key);
    let year = field_value(input, year_key);
    view! {
        <label class="bir-box span-2">{format!("{item} {label}")}
            <div class="date-inputs">
                <input data-form-field=month_key aria-label="Month" inputmode="numeric" maxlength="2" prop:value=month prop:readonly=locked on:input=move |ev| update_field_digits(set_form_input_text, month_key, event_target_value(&ev), 2) />
                <span>"/"</span>
                <input data-form-field=day_key aria-label="Day" inputmode="numeric" maxlength="2" prop:value=day prop:readonly=locked on:input=move |ev| update_field_digits(set_form_input_text, day_key, event_target_value(&ev), 2) />
                <span>"/"</span>
                <input data-form-field=year_key aria-label="Year" inputmode="numeric" maxlength="4" prop:value=year prop:readonly=locked on:input=move |ev| update_field_digits(set_form_input_text, year_key, event_target_value(&ev), 4) />
            </div>
        </label>
    }.into_view()
}

fn render_1701q_choice_box(
    item: &'static str,
    label: &'static str,
    choices: Vec<(&'static str, &'static str)>,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
    special_rule: Option<&'static str>,
) -> View {
    let keys: Vec<&'static str> = choices.iter().map(|(_, key)| *key).collect();
    let group_name = format!("1701q-{item}");
    let box_class = if item == "2" {
        "bir-box checkbox-pair radio-group-1701q"
    } else {
        "bir-box span-2 checkbox-pair radio-group-1701q"
    };
    view! {
        <fieldset class=box_class>
            <legend>{format!("{item} {label}")}</legend>
            {choices.into_iter().map(|(choice_label, key)| {
                let checked = field_bool(input, key);
                let all_keys = keys.clone();
                let radio_name = group_name.clone();
                view! {
                    <label><input type="radio" name=radio_name prop:checked=checked prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) {
                        if special_rule == Some("taxpayer_type") { update_1701q_taxpayer_type(set_form_input_text, all_keys.clone(), key) } else { update_choice_fields(set_form_input_text, all_keys.clone(), key) }
                    } />{choice_label}</label>
                }
            }).collect_view()}
        </fieldset>
    }.into_view()
}

fn render_1701q_atc_box(
    item: &'static str,
    spouse: bool,
    force_business_graduated: bool,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let choices = vec![
        ("II012 Business Income-Graduated IT Rates", if spouse { "ui1701q:spouse_atc_ii012" } else { "ui1701q:taxpayer_atc_ii012" }, false),
        ("II014 Income from Profession-Graduated IT Rates", if spouse { "ui1701q:spouse_atc_ii014" } else { "ui1701q:taxpayer_atc_ii014" }, force_business_graduated),
        ("II013 Mixed Income-Graduated IT Rates", if spouse { "ui1701q:spouse_atc_ii013" } else { "ui1701q:taxpayer_atc_ii013" }, force_business_graduated),
        ("II015 Business Income-8% IT Rate", if spouse { "ui1701q:spouse_atc_ii015" } else { "ui1701q:taxpayer_atc_ii015" }, force_business_graduated),
        ("II017 Income from Profession-8% IT Rate", if spouse { "ui1701q:spouse_atc_ii017" } else { "ui1701q:taxpayer_atc_ii017" }, force_business_graduated),
        ("II016 Mixed Income-8% IT Rate", if spouse { "ui1701q:spouse_atc_ii016" } else { "ui1701q:taxpayer_atc_ii016" }, force_business_graduated),
    ];
    let group_name = if spouse { "1701q-spouse-atc" } else { "1701q-taxpayer-atc" };
    view! {
        <fieldset class="bir-box span-2 checkbox-pair atc-choice-box radio-group-1701q">
            <legend>{format!("{item} Alphanumeric Tax Code (ATC)")}</legend>
            {choices.into_iter().map(|(choice_label, key, disabled)| {
                let checked = field_bool(input, key) || (force_business_graduated && key.ends_with("ii012"));
                view! {
                    <label><input type="radio" name=group_name prop:checked=checked prop:disabled=locked || disabled on:change=move |ev| if event_target_checked(&ev) { update_1701q_atc(set_form_input_text, spouse, key) } />{choice_label}</label>
                }
            }).collect_view()}
        </fieldset>
    }.into_view()
}

fn render_1701q_tax_rate_box(
    item: &'static str,
    spouse: bool,
    eight_percent: bool,
    _input: &Value,
    set_form_input_text: WriteSignal<String>,
    _locked: bool,
) -> View {
    let graduated_key = if spouse { "ui1701q:spouse_rate_graduated" } else { "ui1701q:taxpayer_rate_graduated" };
    let eight_key = if spouse { "ui1701q:spouse_rate_8" } else { "ui1701q:taxpayer_rate_8" };
    let graduated_checked = !eight_percent;
    let eight_checked = eight_percent;
    let group_name = if spouse { "1701q-spouse-rate" } else { "1701q-taxpayer-rate" };
    view! {
        <fieldset class="bir-box span-2 checkbox-pair rate-choice-box radio-group-1701q">
            <legend>{format!("{item} Tax Rate* (choose one, for income from business/profession)")}</legend>
            <label><input type="radio" name=group_name prop:checked=graduated_checked prop:disabled=true on:change=move |ev| if event_target_checked(&ev) { update_choice_fields(set_form_input_text, vec![graduated_key, eight_key], graduated_key) } />"Graduated Rates per Tax Table - page 2 (Choose Method of Deduction in Item 16A/25A)"</label>
            <label><input type="radio" name=group_name prop:checked=eight_checked prop:disabled=true on:change=move |ev| if event_target_checked(&ev) { update_choice_fields(set_form_input_text, vec![graduated_key, eight_key], eight_key) } />"8% on gross sales/receipts & other non-operating income in lieu of Graduated Rates under Sec. 24(A)(2)(a) & Percentage Tax under Sec. 116 of the NIRC, as amended [available if gross sales/receipts and other non-operating income do not exceed P3M]"</label>
        </fieldset>
    }.into_view()
}

fn render_1701q_method_box(
    item: &'static str,
    label: &'static str,
    itemized_key: &'static str,
    osd_key: &'static str,
    disabled_by_rate: bool,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let group_name = format!("1701q-{item}");
    view! {
        <fieldset class="bir-box span-2 checkbox-pair radio-group-1701q">
            <legend>{format!("{item} {label}")}</legend>
            <label><input type="radio" name=group_name.clone() prop:checked=field_bool(input, itemized_key) prop:disabled=locked || disabled_by_rate on:change=move |ev| if event_target_checked(&ev) { update_choice_fields(set_form_input_text, vec![itemized_key, osd_key], itemized_key) } />"Itemized Deduction [Sec. 34(A-J), NIRC]"</label>
            <label><input type="radio" name=group_name prop:checked=field_bool(input, osd_key) prop:disabled=locked || disabled_by_rate on:change=move |ev| if event_target_checked(&ev) { update_choice_fields(set_form_input_text, vec![itemized_key, osd_key], osd_key) } />"Optional Standard Deduction (OSD) [40% of Gross Sales/Receipts/Revenues/Fees [Sec. 34(L), NIRC]]"</label>
        </fieldset>
    }.into_view()
}

fn render_1701q_aggregate_amount_row(
    item: &'static str,
    label: &'static str,
    key: &'static str,
    input: &Value,
    _set_form_input_text: WriteSignal<String>,
    _locked: bool,
) -> View {
    let value = field_value(input, key);
    view! {
        <label class="bir-row bir-row-two-col aggregate-row-1701q">
            <span class="item-no">{item}</span><span class="item-label">{label}</span>
            <input class="amount-input aggregate-amount calculated-amount" prop:value=value readonly />
        </label>
    }.into_view()
}

fn render_1701q_disabled_payment_row(item: &'static str, label: &'static str) -> View {
    view! {
        <div class="payment-row disabled-payment-row">
            <strong>{format!("{item} {label}")}</strong>
            <input disabled />
            <input disabled />
            <input disabled />
            <input disabled />
        </div>
    }.into_view()
}

fn render_1701q_tax_rate_tables() -> View {
    let rows_2018 = [
        ("Not over ₱250,000", "0%"),
        ("Over ₱250,000 but not over ₱400,000", "20% of excess over ₱250,000"),
        ("Over ₱400,000 but not over ₱800,000", "₱30,000 + 25% of excess over ₱400,000"),
        ("Over ₱800,000 but not over ₱2,000,000", "₱130,000 + 30% of excess over ₱800,000"),
        ("Over ₱2,000,000 but not over ₱8,000,000", "₱490,000 + 32% of excess over ₱2,000,000"),
        ("Over ₱8,000,000", "₱2,410,000 + 35% of excess over ₱8,000,000"),
    ];
    let rows_2023 = [
        ("Not over ₱250,000", "0%"),
        ("Over ₱250,000 but not over ₱400,000", "15% of excess over ₱250,000"),
        ("Over ₱400,000 but not over ₱800,000", "₱22,500 + 20% of excess over ₱400,000"),
        ("Over ₱800,000 but not over ₱2,000,000", "₱102,500 + 25% of excess over ₱800,000"),
        ("Over ₱2,000,000 but not over ₱8,000,000", "₱402,500 + 30% of excess over ₱2,000,000"),
        ("Over ₱8,000,000", "₱2,202,500 + 35% of excess over ₱8,000,000"),
    ];
    view! {
        <div class="tax-rate-tables-1701q">
            {render_1701q_tax_rate_table("Table 1 – Tax Rates (effective January 1, 2018 to December 31, 2022)", rows_2018)}
            {render_1701q_tax_rate_table("Table 2 – Tax Rates (effective January 1, 2023 and onwards)", rows_2023)}
        </div>
    }.into_view()
}

fn render_1701q_tax_rate_table(title: &'static str, rows: [(&'static str, &'static str); 6]) -> View {
    view! {
        <div class="tax-rate-table-1701q">
            <strong>{title}</strong>
            <div class="tax-rate-head"><span>"If Taxable Income is:"</span><span>"Tax Due is:"</span></div>
            {rows.into_iter().map(|(income, due)| view! { <div class="tax-rate-row"><span>{income}</span><span>{due}</span></div> }).collect_view()}
        </div>
    }.into_view()
}

fn render_1701q_two_amount_row(
    item: &'static str,
    label: &'static str,
    key_a: &'static str,
    key_b: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    render_1701q_two_amount_row_disabled(item, label, key_a, key_b, false, false, input, set_form_input_text, locked)
}

fn render_1701q_two_amount_row_disabled(
    item: &'static str,
    label: &'static str,
    key_a: &'static str,
    key_b: &'static str,
    disabled_a: bool,
    disabled_b: bool,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let value_a = field_value(input, key_a);
    let value_b = field_value(input, key_b);
    let calculated = is_calculated_box(item);
    view! {
        <label class="bir-row bir-row-two-col">
            <span class="item-no">{item}</span><span class="item-label">{label}</span>
            <input data-form-field=key_a class="amount-input" class:rate-disabled=disabled_a class:calculated-amount=calculated prop:value=value_a prop:readonly=locked || calculated prop:disabled=disabled_a on:input=move |ev| update_field_amount(set_form_input_text, key_a, event_target_value(&ev)) />
            <input data-form-field=key_b class="amount-input" class:rate-disabled=disabled_b class:calculated-amount=calculated prop:value=value_b prop:readonly=locked || calculated prop:disabled=disabled_b on:input=move |ev| update_field_amount(set_form_input_text, key_b, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_1701q_specify_amount_row(
    item: &'static str,
    label: &'static str,
    specify_key: &'static str,
    key_a: &'static str,
    key_b: &'static str,
    disabled_a: bool,
    disabled_b: bool,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let specify = field_value(input, specify_key);
    let value_a = field_value(input, key_a);
    let value_b = field_value(input, key_b);
    view! {
        <label class="bir-row bir-row-two-col specify-row-1701q">
            <span class="item-no">{item}</span><span class="item-label">{label}</span>
            <input data-form-field=specify_key class="specify-input" placeholder="specify" class:rate-disabled=disabled_a && disabled_b prop:value=specify prop:readonly=locked prop:disabled=disabled_a && disabled_b on:input=move |ev| update_field_string(set_form_input_text, specify_key, event_target_value(&ev)) />
            <input data-form-field=key_a class="amount-input" class:rate-disabled=disabled_a prop:value=value_a prop:readonly=locked prop:disabled=disabled_a on:input=move |ev| update_field_amount(set_form_input_text, key_a, event_target_value(&ev)) />
            <input data-form-field=key_b class="amount-input" class:rate-disabled=disabled_b prop:value=value_b prop:readonly=locked prop:disabled=disabled_b on:input=move |ev| update_field_amount(set_form_input_text, key_b, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn update_field_digits(set_form_input_text: WriteSignal<String>, key: &str, value: String, max_digits: usize) {
    let normalized: String = value.chars().filter(|ch| ch.is_ascii_digit()).take(max_digits).collect();
    update_field_string(set_form_input_text, key, normalized);
}

fn update_field_amount(set_form_input_text: WriteSignal<String>, key: &str, value: String) {
    let mut normalized = String::new();
    let mut dot_seen = false;
    let mut centavos_digits = 0usize;
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            if dot_seen {
                if centavos_digits >= 2 {
                    continue;
                }
                centavos_digits += 1;
            }
            normalized.push(ch);
        } else if ch == ',' && !dot_seen {
            normalized.push(ch);
        } else if ch == '.' && !dot_seen {
            dot_seen = true;
            normalized.push(ch);
        } else if ch == '-' && normalized.is_empty() {
            normalized.push(ch);
        }
    }
    update_field_string(set_form_input_text, key, normalized);
}

fn recalculate_1701q_payload(root: &mut Value) {
    let year = root
        .get("fields")
        .and_then(Value::as_object)
        .and_then(|fields| fields.get("frm1701q:txtYear"))
        .map(value_to_form_string)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(2023);
    let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) else {
        return;
    };
    if !fields.contains_key("frm1701q:txtYear") {
        return;
    }

    let mut calculated_columns = [[0_i64; MAX_BOX_NUMBER + 1]; 2];
    for (column_index, column) in ['A', 'B'].into_iter().enumerate() {
        for item in 1..=MAX_BOX_NUMBER {
            if let Some(key) = box_field_key(item, column) {
                calculated_columns[column_index][item] = fields
                    .get(key)
                    .map(value_to_form_string)
                    .map(|value| parse_money_to_centavos(&value))
                    .unwrap_or(0);
            }
        }

        let spouse = column == 'B';
        let eight_percent = if spouse {
            [
                "ui1701q:spouse_atc_ii015",
                "ui1701q:spouse_atc_ii017",
                "ui1701q:spouse_atc_ii016",
            ]
            .iter()
            .any(|key| fields.get(*key).map(value_to_form_string).as_deref() == Some("true"))
        } else {
            [
                "ui1701q:taxpayer_atc_ii015",
                "ui1701q:taxpayer_atc_ii017",
                "ui1701q:taxpayer_atc_ii016",
            ]
            .iter()
            .any(|key| fields.get(*key).map(value_to_form_string).as_deref() == Some("true"))
        };
        let osd_key = if spouse {
            "frm1701:optMethodOfDeduction24:_2"
        } else {
            "frm1701:optMethodOfDeduction23:_2"
        };
        let deduction_method = if fields
            .get(osd_key)
            .map(value_to_form_string)
            .as_deref()
            == Some("true")
        {
            DeductionMethod::OptionalStandard
        } else {
            DeductionMethod::Itemized
        };
        calculate_column(
            year,
            !eight_percent,
            deduction_method,
            &mut calculated_columns[column_index],
        );
    }

    for (column_index, column) in ['A', 'B'].into_iter().enumerate() {
        for item in 1..=MAX_BOX_NUMBER {
            if is_calculated_box(&item.to_string()) && item != 31 {
                if let Some(key) = box_field_key(item, column) {
                    fields.insert(
                        key.to_string(),
                        Value::String(format_centavos(calculated_columns[column_index][item])),
                    );
                }
            }
        }
    }
    let aggregate = aggregate_amount_payable(calculated_columns[0][30], calculated_columns[1][30]);
    fields.insert(
        "frm1701q:txt31A".to_string(),
        Value::String(format_centavos(aggregate)),
    );
}

fn box_field_key(item: usize, column: char) -> Option<&'static str> {
    match (item, column) {
        (26, 'A') => Some("frm1701q:txt26A"), (26, 'B') => Some("frm1701q:txt26B"),
        (27, 'A') => Some("frm1701q:txt27A"), (27, 'B') => Some("frm1701q:txt27B"),
        (28, 'A') => Some("frm1701q:txt28A"), (28, 'B') => Some("frm1701q:txt28B"),
        (29, 'A') => Some("frm1701q:txt29A"), (29, 'B') => Some("frm1701q:txt29B"),
        (30, 'A') => Some("frm1701q:txt30A"), (30, 'B') => Some("frm1701q:txt30B"),
        (36, 'A') => Some("frm1701q:txt36A"), (36, 'B') => Some("frm1701q:txt36B"),
        (37, 'A') => Some("frm1701q:txt37A"), (37, 'B') => Some("frm1701q:txt37B"),
        (38, 'A') => Some("frm1701q:txt38A"), (38, 'B') => Some("frm1701q:txt38B"),
        (39, 'A') => Some("frm1701q:txt38C"), (39, 'B') => Some("frm1701q:txt38D"),
        (40, 'A') => Some("frm1701q:txt38E"), (40, 'B') => Some("frm1701q:txt38F"),
        (41, 'A') => Some("frm1701q:txt38G"), (41, 'B') => Some("frm1701q:txt38H"),
        (42, 'A') => Some("frm1701q:txt38I"), (42, 'B') => Some("frm1701q:txt38J"),
        (43, 'A') => Some("frm1701q:txt38K"), (43, 'B') => Some("frm1701q:txt38L"),
        (44, 'A') => Some("frm1701q:txt38M"), (44, 'B') => Some("frm1701q:txt38N"),
        (45, 'A') => Some("frm1701q:txt39A"), (45, 'B') => Some("frm1701q:txt39B"),
        (46, 'A') => Some("ui1701q:txt46A"), (46, 'B') => Some("ui1701q:txt46B"),
        (47, 'A') => Some("frm1701q:txt40A"), (47, 'B') => Some("frm1701q:txt40B"),
        (48, 'A') => Some("frm1701q:txt40C"), (48, 'B') => Some("frm1701q:txt40D"),
        (49, 'A') => Some("frm1701q:txt40E"), (49, 'B') => Some("frm1701q:txt40F"),
        (50, 'A') => Some("frm1701q:txt40G"), (50, 'B') => Some("frm1701q:txt40H"),
        (51, 'A') => Some("frm1701q:txt41A"), (51, 'B') => Some("frm1701q:txt41B"),
        (52, 'A') => Some("ui1701q:txt52A"), (52, 'B') => Some("ui1701q:txt52B"),
        (53, 'A') => Some("ui1701q:txt53A"), (53, 'B') => Some("ui1701q:txt53B"),
        (54, 'A') => Some("ui1701q:txt54A"), (54, 'B') => Some("ui1701q:txt54B"),
        (55, 'A') => Some("ui1701q:txt55A"), (55, 'B') => Some("ui1701q:txt55B"),
        (56, 'A') => Some("ui1701q:txt56A"), (56, 'B') => Some("ui1701q:txt56B"),
        (57, 'A') => Some("ui1701q:txt57A"), (57, 'B') => Some("ui1701q:txt57B"),
        (58, 'A') => Some("ui1701q:txt58A"), (58, 'B') => Some("ui1701q:txt58B"),
        (59, 'A') => Some("ui1701q:txt59A"), (59, 'B') => Some("ui1701q:txt59B"),
        (60, 'A') => Some("ui1701q:txt60A"), (60, 'B') => Some("ui1701q:txt60B"),
        (61, 'A') => Some("ui1701q:txt61A"), (61, 'B') => Some("ui1701q:txt61B"),
        (62, 'A') => Some("ui1701q:txt62A"), (62, 'B') => Some("ui1701q:txt62B"),
        (63, 'A') => Some("ui1701q:txt63A"), (63, 'B') => Some("ui1701q:txt63B"),
        (64, 'A') => Some("ui1701q:txt64A"), (64, 'B') => Some("ui1701q:txt64B"),
        (65, 'A') => Some("ui1701q:txt65A"), (65, 'B') => Some("ui1701q:txt65B"),
        (66, 'A') => Some("ui1701q:txt66A"), (66, 'B') => Some("ui1701q:txt66B"),
        (67, 'A') => Some("ui1701q:txt67A"), (67, 'B') => Some("ui1701q:txt67B"),
        (68, 'A') => Some("ui1701q:txt68A"), (68, 'B') => Some("ui1701q:txt68B"),
        _ => None,
    }
}

fn update_1701q_taxpayer_type(set_form_input_text: WriteSignal<String>, keys: Vec<&str>, selected_key: &str) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                for key in &keys {
                    fields.insert((*key).to_string(), Value::String((*key == selected_key).to_string()));
                }
                if selected_key.ends_with("estate") || selected_key.ends_with("trust") {
                    apply_1701q_atc_fields(fields, false, "ui1701q:taxpayer_atc_ii012");
                }
            }
            recalculate_1701q_payload(&mut root);
            *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
        }
    });
}

fn update_1701q_atc(set_form_input_text: WriteSignal<String>, spouse: bool, selected_key: &str) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                apply_1701q_atc_fields(fields, spouse, selected_key);
            }
            recalculate_1701q_payload(&mut root);
            *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
        }
    });
}

fn apply_1701q_atc_fields(fields: &mut serde_json::Map<String, Value>, spouse: bool, selected_key: &str) {
    let prefix = if spouse { "ui1701q:spouse_atc_" } else { "ui1701q:taxpayer_atc_" };
    for suffix in ["ii012", "ii014", "ii013", "ii015", "ii017", "ii016"] {
        let key = format!("{prefix}{suffix}");
        fields.insert(key.clone(), Value::String((key == selected_key).to_string()));
    }
    let selected_code = selected_key.rsplit('_').next().unwrap_or("ii012").to_ascii_uppercase();
    let graduated = matches!(selected_code.as_str(), "II012" | "II014" | "II013");
    let slot = match selected_code.as_str() {
        "II014" | "II017" => 2,
        "II013" | "II016" => 3,
        _ => 1,
    };
    let (txt_a, opt_a, txt_b, opt_b, txt_c, opt_c, rate_grad, rate_8, method_1, method_2) = if spouse {
        ("frm1701q:txt22A", "frm1701q:optATC22_1", "frm1701q:txt22B", "frm1701q:optATC22_2", "frm1701q:txt22C", "frm1701q:optATC22_3", "ui1701q:spouse_rate_graduated", "ui1701q:spouse_rate_8", "frm1701:optMethodOfDeduction24:_1", "frm1701:optMethodOfDeduction24:_2")
    } else {
        ("frm1701q:txt20A", "frm1701q:optATC20_1", "frm1701q:txt20B", "frm1701q:optATC20_2", "frm1701q:txt20C", "frm1701q:optATC20_3", "ui1701q:taxpayer_rate_graduated", "ui1701q:taxpayer_rate_8", "frm1701:optMethodOfDeduction23:_1", "frm1701:optMethodOfDeduction23:_2")
    };
    fields.insert(txt_a.to_string(), Value::String(if slot == 1 { selected_code.clone() } else { String::new() }));
    fields.insert(txt_b.to_string(), Value::String(if slot == 2 { selected_code.clone() } else { String::new() }));
    fields.insert(txt_c.to_string(), Value::String(if slot == 3 { selected_code.clone() } else { String::new() }));
    fields.insert(opt_a.to_string(), Value::String((slot == 1).to_string()));
    fields.insert(opt_b.to_string(), Value::String((slot == 2).to_string()));
    fields.insert(opt_c.to_string(), Value::String((slot == 3).to_string()));
    fields.insert(rate_grad.to_string(), Value::String(graduated.to_string()));
    fields.insert(rate_8.to_string(), Value::String((!graduated).to_string()));
    if !graduated {
        fields.insert(method_1.to_string(), Value::String("false".to_string()));
        fields.insert(method_2.to_string(), Value::String("false".to_string()));
    }
    clear_1701q_inactive_schedule(fields, spouse, graduated);
}

fn clear_1701q_inactive_schedule(
    fields: &mut serde_json::Map<String, Value>,
    spouse: bool,
    graduated: bool,
) {
    let column = if spouse { 'B' } else { 'A' };
    let inactive_items: std::ops::RangeInclusive<usize> = if graduated { 47..=54 } else { 36..=46 };
    for item in inactive_items {
        if let Some(key) = box_field_key(item, column) {
            fields.insert(key.to_string(), Value::String("0.00".to_string()));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PhysicalSectionKind {
    Header,
    Background,
    Computation,
    VatComputation,
    Payment,
    Schedule,
}

fn fields_for_physical_section(
    form_code: &str,
    fields: &[(String, String)],
    section: PhysicalSectionKind,
) -> Vec<(String, String)> {
    fields
        .iter()
        .filter(|(key, _)| {
            is_renderable_physical_field(form_code, key)
                && classify_physical_field(form_code, key) == section
        })
        .cloned()
        .collect()
}

fn render_pdf_header_controls(
    form_code: &str,
    input: &Value,
    fallback_fields: Vec<(String, String)>,
    locked: bool,
    set_form_input_text: WriteSignal<String>,
) -> View {
    match form_code {
        "0619E" => view! {
            {render_physical_month_year_box("1", "For the Month", "frm0619E:txtMonth", "frm0619E:txtYear", true, input, set_form_input_text, locked)}
            {render_physical_date_box("2", "Due Date", "frm0619E:txtDueMonth", "frm0619E:txtDueDay", "frm0619E:txtDueYear", input, set_form_input_text, locked)}
            {render_physical_pair_box("3", "Amended Form?", "frm0619E:optAmend:Y", "frm0619E:optAmend:N", input, set_form_input_text, locked)}
            {render_physical_pair_box("4", "Any Taxes Withheld?", "frm0619E:optWithheld:Y", "frm0619E:optWithheld:N", input, set_form_input_text, locked)}
            {render_physical_field_box_dynamic("5", "ATC", "frm0619E:txtAtc", input, set_form_input_text, locked)}
            {render_physical_field_box_dynamic("6", "Tax Type Code", "frm0619E:txtTaxTypeCode", input, set_form_input_text, locked)}
        }.into_view(),
        "1601EQ" => view! {
            {render_physical_field_box_dynamic("1", "For the Year", "frm1601EQ:txtYear", input, set_form_input_text, locked)}
            {render_physical_choice_box("2", "Quarter", vec![("1st", "frm1601EQ:optQuarter:1"), ("2nd", "frm1601EQ:optQuarter:2"), ("3rd", "frm1601EQ:optQuarter:3"), ("4th", "frm1601EQ:optQuarter:4")], input, set_form_input_text, locked)}
            {render_physical_pair_box("3", "Amended Return?", "frm1601EQ:optAmend:Y", "frm1601EQ:optAmend:N", input, set_form_input_text, locked)}
            {render_physical_pair_box("4", "Any Taxes Withheld?", "frm1601EQ:optWithheld:Y", "frm1601EQ:optWithheld:N", input, set_form_input_text, locked)}
            {render_physical_field_box_dynamic("5", "No. of Sheet/s Attached", "frm1601EQ:txtNoSheets", input, set_form_input_text, locked)}
        }.into_view(),
        "1701Q" => view! {
            {render_physical_field_box_dynamic("1", "For the Year", "frm1701q:txtYear", input, set_form_input_text, locked)}
            {render_physical_choice_box("2", "Quarter", vec![("1st", "frm1701q:DateQuarter_1"), ("2nd", "frm1701q:DateQuarter_2"), ("3rd", "frm1701q:DateQuarter_3")], input, set_form_input_text, locked)}
            {render_physical_pair_box("3", "Amended Return?", "frm1701q:AmendedRtn_1", "frm1701q:AmendedRtn_2", input, set_form_input_text, locked)}
            {render_physical_field_box_dynamic("4", "No. of Sheet/s Attached", "frm1701q:txtSheets", input, set_form_input_text, locked)}
        }.into_view(),
        "1702Q" => view! {
            {render_physical_choice_box("1", "For", vec![("Calendar", "frm1702q:rbForClndrFscl_1"), ("Fiscal", "frm1702q:rbForClndrFscl_2")], input, set_form_input_text, locked)}
            {render_physical_month_year_box("2", "Year Ended", "frm1702q:rbYrEndMonth", "frm1702q:txtYrEndYear", false, input, set_form_input_text, locked)}
            {render_physical_choice_box("3", "Quarter", vec![("1st", "frm1702q:rbQuarter_1"), ("2nd", "frm1702q:rbQuarter_2"), ("3rd", "frm1702q:rbQuarter_3")], input, set_form_input_text, locked)}
            {render_physical_pair_box("4", "Amended Return?", "frm1702q:rbAmendedRtn_1", "frm1702q:rbAmendedRtn_2", input, set_form_input_text, locked)}
            {render_physical_atc_1702q_box(input, set_form_input_text, locked)}
        }.into_view(),
        "2000" => view! {
            {render_physical_month_year_box("1", "For the Month", "frm2000:txtMonth", "frm2000:txtYear", true, input, set_form_input_text, locked)}
            {render_physical_pair_box("2", "Amended Return?", "frm2000:AmendedRtn_1", "frm2000:AmendedRtn_2", input, set_form_input_text, locked)}
            {render_physical_field_box_dynamic("3", "No. of Sheet/s Attached", "frm2000:txtSheets", input, set_form_input_text, locked)}
        }.into_view(),
        "2550Q" => view! {
            {render_physical_choice_box("1", "For", vec![("Calendar", "frm2550qv2024:calendarNo1"), ("Fiscal", "frm2550qv2024:fiscalNo1")], input, set_form_input_text, locked)}
            {render_physical_month_year_box("2", "Year Ended", "frm2550qv2024:selectedMonthNo2", "frm2550qv2024:txtYearNo2", true, input, set_form_input_text, locked)}
            {render_physical_choice_box("3", "Quarter", vec![("1st", "frm2550qv2024:OptQuarter1"), ("2nd", "frm2550qv2024:OptQuarter2"), ("3rd", "frm2550qv2024:OptQuarter3"), ("4th", "frm2550qv2024:OptQuarter4")], input, set_form_input_text, locked)}
            {render_physical_period_range_box("4", "Return Period", "frm2550qv2024:RtnPeriodFromNo4", "frm2550qv2024:RtnPeriodToNo4", input, set_form_input_text, locked)}
            {render_physical_pair_box("5", "Amended Return?", "frm2550qv2024:amendedReturnYesNo5", "frm2550qv2024:amendedReturnNo5", input, set_form_input_text, locked)}
            {render_physical_pair_box("6", "Short Period Return?", "frm2550qv2024:OptShortPrd1", "frm2550qv2024:OptShortPrd2", input, set_form_input_text, locked)}
        }.into_view(),
        _ => render_physical_boxes(form_code, fallback_fields, locked, set_form_input_text),
    }
}

fn render_physical_field_box_dynamic(
    item: &'static str,
    label: &'static str,
    key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let value = field_value(input, key);
    view! {
        <label class="bir-box">{format!("{item} {label}")}
            <input data-form-field=key prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, key, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_physical_month_year_box(
    item: &'static str,
    label: &'static str,
    month_key: &'static str,
    year_key: &'static str,
    sync_return_period: bool,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let month = field_value(input, month_key);
    let year = field_value(input, year_key);
    view! {
        <label class="bir-box">{format!("{item} {label} (MM/YYYY)")}
            <div class="split-inputs">
                <input data-form-field=month_key aria-label="Month" prop:value=month prop:readonly=locked on:input=move |ev| update_period_component(set_form_input_text, month_key, "month", sync_return_period, event_target_value(&ev)) />
                <span>"/"</span>
                <input data-form-field=year_key aria-label="Year" prop:value=year prop:readonly=locked on:input=move |ev| update_period_component(set_form_input_text, year_key, "year", sync_return_period, event_target_value(&ev)) />
            </div>
        </label>
    }.into_view()
}

fn render_physical_date_box(
    item: &'static str,
    label: &'static str,
    month_key: &'static str,
    day_key: &'static str,
    year_key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let month = field_value(input, month_key);
    let day = field_value(input, day_key);
    let year = field_value(input, year_key);
    view! {
        <label class="bir-box span-2">{format!("{item} {label} (MM/DD/YYYY)")}
            <div class="date-inputs">
                <input data-form-field=month_key aria-label="Month" prop:value=month prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, month_key, event_target_value(&ev)) />
                <span>"/"</span>
                <input data-form-field=day_key aria-label="Day" prop:value=day prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, day_key, event_target_value(&ev)) />
                <span>"/"</span>
                <input data-form-field=year_key aria-label="Year" prop:value=year prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, year_key, event_target_value(&ev)) />
            </div>
        </label>
    }.into_view()
}

fn render_physical_period_range_box(
    item: &'static str,
    label: &'static str,
    from_key: &'static str,
    to_key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let from = field_value(input, from_key);
    let to = field_value(input, to_key);
    view! {
        <label class="bir-box span-2">{format!("{item} {label}")}
            <div class="period-range-inputs">
                <input data-form-field=from_key aria-label="From" placeholder="From MM/DD/YYYY" prop:value=from prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, from_key, event_target_value(&ev)) />
                <span>"to"</span>
                <input data-form-field=to_key aria-label="To" placeholder="To MM/DD/YYYY" prop:value=to prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, to_key, event_target_value(&ev)) />
            </div>
        </label>
    }.into_view()
}

fn render_physical_pair_box(
    item: &'static str,
    label: &'static str,
    yes_key: &'static str,
    no_key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let yes = field_bool(input, yes_key);
    let no = field_bool(input, no_key);
    view! {
        <div class="bir-box checkbox-pair">
            <span>{format!("{item} {label}")}</span>
            <label><input type="checkbox" prop:checked=yes prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) { update_pair_fields(set_form_input_text, yes_key, no_key, true) } />"Yes"</label>
            <label><input type="checkbox" prop:checked=no prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) { update_pair_fields(set_form_input_text, yes_key, no_key, false) } />"No"</label>
        </div>
    }.into_view()
}

fn render_physical_choice_box(
    item: &'static str,
    label: &'static str,
    choices: Vec<(&'static str, &'static str)>,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let keys: Vec<&'static str> = choices.iter().map(|(_, key)| *key).collect();
    view! {
        <div class="bir-box checkbox-pair">
            <span>{format!("{item} {label}")}</span>
            {choices.into_iter().map(|(choice_label, key)| {
                let checked = field_bool(input, key);
                let all_keys = keys.clone();
                view! {
                    <label><input type="checkbox" prop:checked=checked prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) { update_choice_fields(set_form_input_text, all_keys.clone(), key) } />{choice_label}</label>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

fn render_physical_atc_1702q_box(
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let regular = field_bool(input, "frm1702q:rbATC_1");
    let special = field_bool(input, "frm1702q:rbATC_2");
    let regular_code = field_value(input, "frm1702q:txtATC_1");
    let special_code = field_value(input, "frm1702q:cbATC_2");
    view! {
        <div class="bir-box span-2 checkbox-pair atc-choice-box">
            <span>"5 Alphanumeric Tax Code (ATC)"</span>
            <label><input type="checkbox" prop:checked=regular prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) { update_choice_fields(set_form_input_text, vec!["frm1702q:rbATC_1", "frm1702q:rbATC_2"], "frm1702q:rbATC_1") } />"Regular / Normal Rate"</label>
            <input data-form-field="frm1702q:txtATC_1" aria-label="Regular rate ATC" placeholder="ATC" prop:value=regular_code prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, "frm1702q:txtATC_1", event_target_value(&ev)) />
            <label><input type="checkbox" prop:checked=special prop:disabled=locked on:change=move |ev| if event_target_checked(&ev) { update_choice_fields(set_form_input_text, vec!["frm1702q:rbATC_1", "frm1702q:rbATC_2"], "frm1702q:rbATC_2") } />"Special Rate"</label>
            <input data-form-field="frm1702q:cbATC_2" aria-label="Special rate ATC" placeholder="ATC" prop:value=special_code prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, "frm1702q:cbATC_2", event_target_value(&ev)) />
        </div>
    }.into_view()
}

fn field_bool(input: &Value, key: &str) -> bool {
    input
        .get("fields")
        .and_then(|fields| fields.get(key))
        .map(|value| match value {
            Value::Bool(value) => *value,
            Value::String(value) => {
                value.eq_ignore_ascii_case("true")
                    || value == "1"
                    || value.eq_ignore_ascii_case("yes")
            }
            _ => false,
        })
        .unwrap_or(false)
}

fn render_physical_boxes(
    form_code: &str,
    fields: Vec<(String, String)>,
    locked: bool,
    set_form_input_text: WriteSignal<String>,
) -> View {
    if fields.is_empty() {
        return view! { <div class="bir-box muted-empty">"No fields in this section for the selected fixture."</div> }.into_view();
    }
    fields.into_iter().map(|(key, value)| {
        let label = physical_field_label(form_code, &key);
        let input_key = key.clone();
        let class_name = if is_wide_physical_field(&key) { "bir-box span-2" } else { "bir-box" };
        view! {
            <label class=class_name>{label}
                <input data-form-field=input_key.clone() prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, &input_key, event_target_value(&ev)) />
            </label>
        }
    }).collect_view().into_view()
}

fn render_physical_rows(
    form_code: &str,
    fields: Vec<(String, String)>,
    locked: bool,
    set_form_input_text: WriteSignal<String>,
) -> View {
    if fields.is_empty() {
        return view! { <div class="bir-subrow">"No computation fields in this section for the selected fixture."</div> }.into_view();
    }
    fields.into_iter().map(|(key, value)| {
        let item = physical_item_no(&key).unwrap_or_else(|| "—".to_string());
        let label = physical_row_label(form_code, &key);
        let input_key = key.clone();
        view! {
            <label class="bir-row">
                <span class="item-no">{item}</span><span class="item-label">{label}</span>
                <input data-form-field=input_key.clone() class="amount-input" prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, &input_key, event_target_value(&ev)) />
            </label>
        }
    }).collect_view().into_view()
}

fn render_physical_payment_rows(
    form_code: &str,
    fields: Vec<(String, String)>,
    locked: bool,
    set_form_input_text: WriteSignal<String>,
) -> View {
    if fields.is_empty() {
        return view! { <div class="bir-subrow">"No payment fields in this section for the selected fixture."</div> }.into_view();
    }
    fields.into_iter().map(|(key, value)| {
        let item = physical_item_no(&key).unwrap_or_else(|| "—".to_string());
        let label = physical_row_label(form_code, &key);
        let input_key = key.clone();
        view! {
            <label class="bir-row payment-field-row">
                <span class="item-no">{item}</span><span class="item-label">{label}</span>
                <input data-form-field=input_key.clone() prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, &input_key, event_target_value(&ev)) />
            </label>
        }
    }).collect_view().into_view()
}

fn classify_physical_field(form_code: &str, key: &str) -> PhysicalSectionKind {
    if let Some(section) = official_physical_section(form_code, key) {
        return section;
    }

    let lower = key.to_ascii_lowercase();
    if lower.contains("sched")
        || lower.contains("schedule")
        || lower.contains("pg2")
        || lower.contains("page2")
    {
        return PhysicalSectionKind::Schedule;
    }
    if lower.contains("tin")
        || lower.contains("branchcode")
        || lower.contains("branch_code")
        || lower.contains("rdocode")
        || lower.contains("taxpayername")
        || lower.contains("taxpayer_name")
        || lower.contains("linebus")
        || lower.contains("address")
        || lower.contains("zipcode")
        || lower.contains("telnum")
        || lower.contains("contact")
        || lower.contains("email")
    {
        return PhysicalSectionKind::Background;
    }
    let item = physical_item_no(key).and_then(|item| {
        item.chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .ok()
    });
    match form_code {
        "0619E" => match item {
            Some(1..=6) | None => PhysicalSectionKind::Header,
            Some(7..=13) => PhysicalSectionKind::Background,
            Some(14..=18) => PhysicalSectionKind::Computation,
            Some(19..=22) => PhysicalSectionKind::Payment,
            _ => PhysicalSectionKind::Schedule,
        },
        "1601EQ" => match item {
            Some(1..=10) | None => PhysicalSectionKind::Header,
            Some(11..=12) => PhysicalSectionKind::Background,
            Some(13..=31) => PhysicalSectionKind::Computation,
            Some(32..=35) => PhysicalSectionKind::Payment,
            _ => PhysicalSectionKind::Schedule,
        },
        "1701Q" => match item {
            Some(1..=4) | None => PhysicalSectionKind::Header,
            Some(5..=25) => PhysicalSectionKind::Background,
            Some(26..=41) => PhysicalSectionKind::Computation,
            _ => PhysicalSectionKind::Schedule,
        },
        "1702Q" => match item {
            Some(1..=12) | None => PhysicalSectionKind::Header,
            Some(13..=25) => PhysicalSectionKind::Background,
            Some(26..=51) => PhysicalSectionKind::Computation,
            Some(52..=55) => PhysicalSectionKind::Payment,
            _ => PhysicalSectionKind::Schedule,
        },
        "2000" => match item {
            Some(1..=5) | None => PhysicalSectionKind::Header,
            Some(6..=20) => PhysicalSectionKind::Background,
            Some(21..=33) => PhysicalSectionKind::Computation,
            Some(34..=37) => PhysicalSectionKind::Payment,
            _ => PhysicalSectionKind::Schedule,
        },
        "2550Q" => match item {
            Some(1..=6) | None => PhysicalSectionKind::Header,
            Some(7..=14) => PhysicalSectionKind::Background,
            Some(15..=30)
                if lower.contains("agency")
                    || lower.contains("number")
                    || lower.contains("date")
                    || lower.contains("amount") && item.unwrap_or(0) >= 27 =>
            {
                PhysicalSectionKind::Payment
            }
            Some(15..=30) => PhysicalSectionKind::Computation,
            Some(31..=61) => PhysicalSectionKind::VatComputation,
            _ => PhysicalSectionKind::Schedule,
        },
        _ => PhysicalSectionKind::Computation,
    }
}

fn physical_field_sort_key(form_code: &str, key: &str) -> String {
    let section = match classify_physical_field(form_code, key) {
        PhysicalSectionKind::Header => 0,
        PhysicalSectionKind::Background => 1,
        PhysicalSectionKind::Computation => 2,
        PhysicalSectionKind::VatComputation => 3,
        PhysicalSectionKind::Payment => 4,
        PhysicalSectionKind::Schedule => 5,
    };
    let item = physical_item_no(key).unwrap_or_else(|| "999".to_string());
    format!("{section:02}:{:>6}:{key}", item)
}

fn physical_item_no(key: &str) -> Option<String> {
    let stem = key.rsplit(':').next().unwrap_or(key);
    let patterns = [
        "No",
        "Tax",
        "Amount",
        "Agency",
        "Number",
        "Date",
        "Particular",
        "Item",
        "Line",
    ];
    for pattern in patterns {
        if let Some(pos) = stem.find(pattern) {
            let tail = &stem[pos + pattern.len()..];
            let item: String = tail
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || ch.is_ascii_uppercase())
                .collect();
            if item.chars().any(|ch| ch.is_ascii_digit()) {
                return Some(item);
            }
        }
    }
    let digits: String = stem
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit() || ch.is_ascii_uppercase())
        .collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

fn official_physical_section(form_code: &str, key: &str) -> Option<PhysicalSectionKind> {
    let stem = key.rsplit(':').next().unwrap_or(key);
    let lower = key.to_ascii_lowercase();
    match form_code {
        "0619E" => {
            if matches!(
                stem,
                "txtMonth"
                    | "txtYear"
                    | "txtDueMonth"
                    | "txtDueDay"
                    | "txtDueYear"
                    | "txtAtc"
                    | "txtTaxTypeCode"
            ) || lower.contains("optamend")
                || lower.contains("optwithheld")
            {
                Some(PhysicalSectionKind::Header)
            } else if matches!(
                stem,
                "txtTIN1"
                    | "txtTIN2"
                    | "txtTIN3"
                    | "txtBranchCode"
                    | "txtRDOCode"
                    | "txtTaxpayerName"
                    | "txtLineBus"
                    | "txtAddress"
                    | "txtZipCode"
                    | "txtTelNum"
                    | "txtEmail"
            ) || lower.contains("optcategory")
            {
                Some(PhysicalSectionKind::Background)
            } else if stem.starts_with("txtTax1") {
                Some(PhysicalSectionKind::Computation)
            } else if lower.contains("agency")
                || lower.contains("number")
                || lower.contains("date")
                || lower.contains("amount")
                || lower.contains("particular")
            {
                Some(PhysicalSectionKind::Payment)
            } else {
                None
            }
        }
        "1601EQ" => {
            if matches!(stem, "txtYear" | "txtNoSheets")
                || lower.contains("optquarter")
                || lower.contains("optamend")
                || lower.contains("optwithheld")
            {
                Some(PhysicalSectionKind::Header)
            } else if matches!(
                stem,
                "txtTIN1"
                    | "txtTIN2"
                    | "txtTIN3"
                    | "txtBranchCode"
                    | "txtRDOCode"
                    | "txtTaxpayerName"
                    | "txtLineBus"
                    | "txtAddress"
                    | "txtZipCode"
                    | "txtTelNum"
                    | "txtEmail"
            ) || lower.contains("optcategory")
            {
                Some(PhysicalSectionKind::Background)
            } else if stem.starts_with("txtAtcCd")
                || stem.starts_with("txtTaxBase")
                || stem.starts_with("txtTaxRate")
                || stem.starts_with("txtTaxbeWithHeld")
                || stem.starts_with("txtTotalOtherTax")
                || stem.starts_with("txtTax")
                || stem.starts_with("if")
            {
                Some(PhysicalSectionKind::Computation)
            } else if lower.contains("agency")
                || lower.contains("number")
                || lower.contains("date")
                || lower.contains("amount")
                || lower.contains("particular")
            {
                Some(PhysicalSectionKind::Payment)
            } else {
                None
            }
        }
        "1701Q" => {
            if matches!(stem, "txtYear" | "DateQuarter_1" | "DateQuarter_2" | "DateQuarter_3" | "AmendedRtn_1" | "AmendedRtn_2" | "txtSheets") {
                Some(PhysicalSectionKind::Header)
            } else if stem.starts_with("txt5")
                || stem.starts_with("txt7")
                || matches!(stem, "txtTaxPayername" | "txtSpousename" | "txt11Address" | "txt12Address" | "txt13BirthMonth" | "txt13BirthDay" | "txt13BirthYear" | "txt14zip" | "txt15Telno" | "txt16BirthMonth" | "txt16BirthDay" | "txt16BirthYear" | "txt17" | "txt18Telno" | "txt19" | "txt20A" | "txt20B" | "txt20C" | "txt21" | "txt22A" | "txt22B" | "txt22C" | "SelTreaty_1" | "SelTreaty_2" | "txtTaxRelief25")
                || lower.contains("optatc20")
                || lower.contains("optatc22")
                || lower.contains("optmethodofdeduction")
            {
                Some(PhysicalSectionKind::Background)
            } else if stem.starts_with("txt") {
                Some(PhysicalSectionKind::Computation)
            } else {
                None
            }
        },
        "1702Q" => {
            if lower.contains("rbforclndrfscl")
                || matches!(stem, "rbYrEndMonth" | "txtYrEndYear")
                || lower.contains("rbquarter")
                || lower.contains("rbamendedrtn")
                || lower.contains("atc")
            {
                Some(PhysicalSectionKind::Header)
            } else if matches!(
                stem,
                "txtTIN1"
                    | "txtTIN2"
                    | "txtTIN3"
                    | "txtBranchCode"
                    | "txtRDOCode"
                    | "txtTaxpayerName1"
                    | "txtAddress"
                    | "txtZipCode"
                    | "txtTelNum"
                    | "txtEmail"
            ) || lower.contains("mthdofddctns")
                || lower.contains("txrlf")
            {
                Some(PhysicalSectionKind::Background)
            } else if stem.starts_with("txtTax") || stem == "txtSheets" {
                Some(PhysicalSectionKind::Computation)
            } else if lower.contains("sched") {
                Some(PhysicalSectionKind::Schedule)
            } else {
                None
            }
        }
        "2000" => {
            if matches!(stem, "txtMonth" | "txtYear" | "txtSheets") || lower.contains("amendedrtn")
            {
                Some(PhysicalSectionKind::Header)
            } else if matches!(
                stem,
                "txtTIN1"
                    | "txtTIN2"
                    | "txtTIN3"
                    | "txtBranchCode"
                    | "txtRDOCode"
                    | "txtTaxpayerName"
                    | "txtAddress"
                    | "txtZipCode"
                    | "txtTelNum"
                    | "txtEmail"
                    | "txtOtherName"
                    | "txtOtherTIN"
            ) || lower.contains("optparty")
                || lower.contains("optmode")
            {
                Some(PhysicalSectionKind::Background)
            } else if stem.starts_with("txtTax1") {
                Some(PhysicalSectionKind::Computation)
            } else if lower.contains("agency")
                || lower.contains("number")
                || lower.contains("date")
                || lower.contains("amount")
                || lower.contains("particular")
            {
                Some(PhysicalSectionKind::Payment)
            } else if lower.contains("sched") {
                Some(PhysicalSectionKind::Schedule)
            } else {
                None
            }
        }
        "2550Q" => {
            if matches!(
                stem,
                "calendarNo1"
                    | "fiscalNo1"
                    | "selectedMonthNo2"
                    | "txtYearNo2"
                    | "RtnPeriodFromNo4"
                    | "RtnPeriodToNo4"
            ) || lower.contains("optquarter")
                || lower.contains("amendedreturn")
                || lower.contains("optshortprd")
            {
                Some(PhysicalSectionKind::Header)
            } else if matches!(
                stem,
                "txtTIN1"
                    | "txtTIN2"
                    | "txtTIN3"
                    | "branchCode"
                    | "txtRDOCode"
                    | "taxpayerName"
                    | "taxpayerAddress"
                    | "taxpayerZip"
                    | "taxpayerContactNumber"
                    | "taxpayerEmailAddress"
                    | "internationalTreatyYn"
                    | "specialRateYn"
                    | "specifyInternationalTreaty"
            ) || lower.contains("taxpayerclassification")
            {
                Some(PhysicalSectionKind::Background)
            } else if matches!(
                stem,
                "excessInputTax"
                    | "creditableVat"
                    | "advVatPayment"
                    | "vatPaidReturn"
                    | "addSpecifyNo19"
                    | "otherCreditsNo19"
                    | "totalTaxCredits"
                    | "excessCredits"
                    | "surcharge"
                    | "interest"
                    | "compromise"
                    | "penalties"
                    | "totalPayable"
            ) {
                Some(PhysicalSectionKind::Computation)
            } else if matches!(
                stem,
                "vatableSales"
                    | "outputVatSales"
                    | "zeroRatedSales"
                    | "exemptSales"
                    | "totalSales"
                    | "outputTaxDue"
                    | "lessOutputVat"
                    | "addOutputVat"
                    | "totalAdjOutput"
                    | "inputTaxCarried"
                    | "inputTaxDeferred"
                    | "transitionalInputTax"
                    | "presumptiveInputTax"
                    | "addSpecifyNo42"
                    | "otherSpecify42"
                    | "total43"
                    | "domesticPurchase"
                    | "domesticInputTax"
                    | "servicesPurchase"
                    | "serviceInputTax"
                    | "importPurchase"
                    | "importInputTax"
                    | "addSpecifyNo47"
                    | "otherSpecify47"
                    | "domesticPurchaseNoTax"
                    | "vatExemptImports"
                    | "totalCurPurchase"
                    | "totalCurInputTax"
                    | "totalAvailInputTax"
                    | "importCapitalInputTax"
                    | "inputTaxAttr"
                    | "vatRefund"
                    | "inputVatUnpaid"
                    | "addSpecifyNo56"
                    | "otherSpecify56"
                    | "totalDeductions"
                    | "addInputVat"
                    | "adjDeductions"
                    | "totalAllowInputTax"
                    | "netVatPayable"
            ) {
                Some(PhysicalSectionKind::VatComputation)
            } else if lower.contains("sched")
                || lower.contains("datepurchase")
                || lower.contains("datecovered")
                || lower.contains("officialreceipt")
                || lower.contains("amountpaid")
            {
                Some(PhysicalSectionKind::Schedule)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn is_renderable_physical_field(form_code: &str, key: &str) -> bool {
    let lower = key.to_ascii_lowercase();

    // XML captures contain desktop-app state and lookup/dropdown backing data that is required
    // for byte-stable XML rendering, but does not exist on the printed BIR PDFs. Keep it in
    // JSON/template mappings, but do not draw it on the physical form surface.
    let internal_markers = [
        "finalflag",
        "enroll",
        "ebironline",
        "secret",
        "driveselect",
        "currentpage",
        "maxpage",
        "modlabel",
        "delete",
        "pg2tin",
        "pg2branch",
        "pg2taxpayer",
        "txtpgtin",
        "txtpg2",
        "resultotherspecify",
    ];
    if internal_markers.iter().any(|marker| lower.contains(marker)) {
        return false;
    }
    if form_code == "1601EQ"
        && (lower.starts_with("atccode")
            || lower.starts_with("atcdesc")
            || lower.starts_with("atctaxrate"))
    {
        return false;
    }
    if form_code == "2000"
        && (lower.starts_with("ds") || lower.contains("numofdays") || lower.contains("numofmonths"))
    {
        return false;
    }

    true
}

fn physical_field_label(form_code: &str, key: &str) -> String {
    let item = physical_item_no(key)
        .map(|item| format!("{item} "))
        .unwrap_or_default();
    format!("{}{}", item, physical_label_without_item(form_code, key))
}

fn physical_row_label(form_code: &str, key: &str) -> String {
    physical_label_without_item(form_code, key)
}

fn physical_label_without_item(form_code: &str, key: &str) -> String {
    if let Some(label) = official_physical_label(form_code, key) {
        return label.to_string();
    }
    heuristic_physical_label(key)
}

fn official_physical_label(form_code: &str, key: &str) -> Option<&'static str> {
    let stem = key.rsplit(':').next().unwrap_or(key);
    match form_code {
        "0619E" => match stem {
            "txtMonth" | "txtYear" => Some("For the Month of (MM/YYYY)"),
            "txtDueMonth" | "txtDueDay" | "txtDueYear" => Some("Due Date (MM/DD/YYYY)"),
            "optAmend" | "Y" | "N" if key.contains("optAmend") => Some("Amended Form?"),
            "optWithheld" if key.contains("optWithheld") => Some("Any Taxes Withheld?"),
            "txtAtc" => Some("ATC"),
            "txtTaxTypeCode" => Some("Tax Type Code"),
            "txtTIN1" | "txtTIN2" | "txtTIN3" | "txtBranchCode" => {
                Some("Taxpayer Identification Number (TIN)")
            }
            "txtRDOCode" => Some("RDO Code"),
            "txtTaxpayerName" => Some("Withholding Agent’s Name"),
            "txtLineBus" => Some("Registered activity / line of business"),
            "txtAddress" => Some("Registered Address"),
            "txtZipCode" => Some("ZIP Code"),
            "txtTelNum" => Some("Contact Number"),
            "optCategory" if key.contains("optCategory") => Some("Category of Withholding Agent"),
            "txtEmail" => Some("Email Address"),
            "txtTax14" => Some("Amount of Remittance"),
            "txtTax15" => Some("Less: Amount Remitted from Previously Filed Form, if amended"),
            "txtTax16" => Some("Net Amount of Remittance (Item 14 Less Item 15)"),
            "txtTax17A" => Some("Surcharge"),
            "txtTax17B" => Some("Interest"),
            "txtTax17C" => Some("Compromise"),
            "txtTax17D" => Some("Total Penalties (Sum of Items 17A to 17C)"),
            "txtTax18" => Some("Total Amount of Remittance (Sum of Items 16 and 17D)"),
            "txtAgency19" | "txtNumber19" | "txtDate19" | "txtAmount19" => {
                Some("Cash/Bank Debit Memo")
            }
            "txtAgency20" | "txtNumber20" | "txtDate20" | "txtAmount20" => Some("Check"),
            "txtAgency21" | "txtNumber21" | "txtDate21" | "txtAmount21" => Some("Tax Debit Memo"),
            "txtParticular22" | "txtAgency22" | "txtNumber22" | "txtDate22" | "txtAmount22" => {
                Some("Others (specify below)")
            }
            _ => None,
        },
        "1601EQ" => match stem {
            "txtYear" => Some("For the Year"),
            "1" | "2" | "3" | "4" if key.contains("optQuarter") => Some("Quarter"),
            "Y" | "N" if key.contains("optAmend") => Some("Amended Return?"),
            "Y" | "N" if key.contains("optWithheld") => Some("Any Taxes Withheld?"),
            "txtNoSheets" => Some("No. of Sheet/s Attached"),
            "txtTIN1" | "txtTIN2" | "txtTIN3" | "txtBranchCode" => {
                Some("Taxpayer Identification Number (TIN)")
            }
            "txtRDOCode" => Some("RDO Code"),
            "txtTaxpayerName" => Some("Withholding Agent’s Name"),
            "txtLineBus" => Some("Registered activity / line of business"),
            "txtAddress" => Some("Registered Address"),
            "txtZipCode" => Some("ZIP Code"),
            "txtTelNum" => Some("Contact Number"),
            "P" | "G" if key.contains("optCategory") => Some("Category of Withholding Agent"),
            "txtEmail" => Some("Email Address"),
            "txtTotalOtherTax" => Some("Total taxes withheld for expanded withholding entries"),
            "txtTax19" => Some("Total Taxes Withheld for the Quarter"),
            "txtTax20" => Some("Less: Remittances Made – 1st Month of the Quarter"),
            "txtTax21" => Some("Less: Remittances Made – 2nd Month of the Quarter"),
            "txtTax22" => Some("Tax Remitted in Return Previously Filed, if amended"),
            "txtTax23" => Some("Over-remittance from Previous Quarter of the same taxable year"),
            "txtTax24" => Some("Other Payments Made"),
            "txtTax25" => Some("Total Remittances Made"),
            "txtTax26" => Some("Tax Still Due/(Over-remittance)"),
            "txtTax27" => Some("Surcharge"),
            "txtTax28" => Some("Interest"),
            "txtTax29" => Some("Compromise"),
            "txtTax30" => Some("Total Penalties"),
            "txtTax31" => Some("TOTAL AMOUNT STILL DUE/(Over-remittance)"),
            "ifRefund" => Some("Over-remittance option: To be Refunded"),
            "ifIssueCert" => Some("Over-remittance option: Tax Credit Certificate"),
            "ifCarriedOver" => Some("Over-remittance option: To be Carried Over"),
            _ if stem.starts_with("txtAtcCd") => Some("ATC Code"),
            _ if stem.starts_with("txtTaxBase") => Some("Tax Base"),
            _ if stem.starts_with("txtTaxRate") => Some("Tax Rate"),
            _ if stem.starts_with("txtTaxbeWithHeld") => Some("Tax Required to be Withheld"),
            _ if stem.contains("Agency32")
                || stem.contains("Number32")
                || stem.contains("Date32")
                || stem.contains("Amount32") =>
            {
                Some("Cash/Bank Debit Memo")
            }
            _ if stem.contains("Agency33")
                || stem.contains("Number33")
                || stem.contains("Date33")
                || stem.contains("Amount33") =>
            {
                Some("Check")
            }
            _ if stem.contains("Agency34")
                || stem.contains("Number34")
                || stem.contains("Date34")
                || stem.contains("Amount34") =>
            {
                Some("Tax Debit Memo")
            }
            _ if stem.contains("Agency35")
                || stem.contains("Number35")
                || stem.contains("Date35")
                || stem.contains("Amount35")
                || stem.contains("Particular35") =>
            {
                Some("Others (specify below)")
            }
            _ => None,
        },
        "1701Q" => match stem {
            "txtYear" => Some("For the Year"),
            "DateQuarter_1" | "DateQuarter_2" | "DateQuarter_3" => Some("Quarter"),
            "AmendedRtn_1" | "AmendedRtn_2" => Some("Amended Return?"),
            "txtSheets" => Some("No. of Sheet/s Attached"),
            "txt5TIN1" | "txt5TIN2" | "txt5TIN3" | "txt5BranchCode" => Some("Taxpayer Identification Number (TIN)"),
            "txt5RDOCode" => Some("RDO Code"),
            "txt7TIN1" | "txt7TIN2" | "txt7TIN3" | "txt7BranchCode" => Some("Spouse TIN"),
            "txt7RDOCode" => Some("Spouse RDO Code"),
            "txtTaxPayername" => Some("Taxpayer Name"),
            "txtSpousename" => Some("Spouse Name"),
            "txt11Address" => Some("Registered Address"),
            "txt12Address" => Some("Spouse Registered Address"),
            "txt13BirthMonth" | "txt13BirthDay" | "txt13BirthYear" => Some("Taxpayer Date of Birth"),
            "txt14zip" => Some("ZIP Code"),
            "txt15Telno" => Some("Taxpayer Contact Number"),
            "txt16BirthMonth" | "txt16BirthDay" | "txt16BirthYear" => Some("Spouse Date of Birth"),
            "txt17" => Some("Spouse ZIP Code"),
            "txt18Telno" => Some("Spouse Contact Number"),
            "txt19" => Some("Registered Activity / Line of Business"),
            "txt20A" | "txt20B" | "txt20C" | "optATC20_1" | "optATC20_2" | "optATC20_3" => Some("Taxpayer ATC"),
            "txt21" => Some("Other ATC / Tax Rate Description"),
            "txt22A" | "txt22B" | "txt22C" | "optATC22_1" | "optATC22_2" | "optATC22_3" => Some("Spouse ATC"),
            "_1" | "_2" if key.contains("optMethodOfDeduction23") => Some("Taxpayer Method of Deduction"),
            "_1" | "_2" if key.contains("optMethodOfDeduction24") => Some("Spouse Method of Deduction"),
            "SelTreaty_1" | "SelTreaty_2" | "txtTaxRelief25" => Some("Tax Relief / Treaty Details"),
            _ => None,
        },
        "1702Q" => match stem {
            "rbForClndrFscl_1" | "rbForClndrFscl_2" => Some("For: Calendar / Fiscal"),
            "rbYrEndMonth" | "txtYrEndYear" => Some("Year Ended (MM/20YY)"),
            "rbQuarter_1" | "rbQuarter_2" | "rbQuarter_3" => Some("Quarter"),
            "rbAmendedRtn_1" | "rbAmendedRtn_2" => Some("Amended Return?"),
            "txtATC_1" | "rbATC_1" | "cbATC_2" | "rbATC_2" => Some("Alphanumeric Tax Code (ATC)"),
            "txtTIN1" | "txtTIN2" | "txtTIN3" | "txtBranchCode" => {
                Some("Taxpayer Identification Number (TIN)")
            }
            "txtRDOCode" => Some("RDO Code"),
            "txtTaxpayerName1" => Some("Registered Name"),
            "txtAddress" => Some("Registered Address"),
            "txtZipCode" => Some("ZIP Code"),
            "txtTelNum" => Some("Contact Number"),
            "txtEmail" => Some("Email Address"),
            "rbMthdOfDdctns_1" | "rbMthdOfDdctns_2" => Some("Method of Deductions"),
            "rbTxRlf_1" | "rbTxRlf_2" | "txtTxRlfSpcfy" => {
                Some("Special Law/International Tax Treaty?")
            }
            "txtTax14" => Some("Income Tax Due – Regular/Normal Rate"),
            "txtTax15" => Some("Less: Share of Other Agencies"),
            "txtTax16" => Some("Balance/Income Tax Still Due – Regular/Normal Rate"),
            "txtTax17" => Some("Add: Income Tax Due – Special Rate"),
            "txtTax18" => Some("Aggregate Income Tax Due"),
            "txtTax19" => Some("Less: Total Tax Credits/Payments"),
            "txtTax20" => Some("Net Tax Payable / (Overpayment)"),
            "txtTax21" => Some("Surcharge"),
            "txtTax22" => Some("Interest"),
            "txtTax23" => Some("Compromise"),
            "txtTax24" => Some("Total Penalties"),
            "txtTax25" => Some("TOTAL AMOUNT PAYABLE / (Overpayment)"),
            "txtSheets" => Some("Number of attached sheets"),
            _ => None,
        },
        "2000" => match stem {
            "txtMonth" | "txtYear" => Some("For the month of (MM/YYYY)"),
            "AmendedRtn_1" | "AmendedRtn_2" => Some("Amended Return?"),
            "txtSheets" => Some("Number of Sheet/s Attached"),
            "txtTIN1" | "txtTIN2" | "txtTIN3" | "txtBranchCode" => {
                Some("Taxpayer Identification Number (TIN)")
            }
            "txtRDOCode" => Some("RDO Code"),
            "txtTaxpayerName" => Some("Taxpayer’s Name"),
            "txtAddress" => Some("Registered Address"),
            "txtZipCode" => Some("ZIP Code"),
            "txtTelNum" => Some("Contact Number"),
            "txtEmail" => Some("Email Address"),
            "optParty_1" | "optParty_2" | "optParty_3" => Some("Other Party to the transaction"),
            "txtOtherName" => Some("Other Party Name"),
            "txtOtherTIN" => Some("Other Party TIN"),
            "optMode_1" | "optMode_2" | "optMode_3" => Some("Mode of Affixture"),
            "txtTax14" => Some("Tax Due for the Month"),
            "txtTax15A" => Some("Less: Tax Paid in Return Previously Filed, if amended"),
            "txtTax15B" => Some("Payment thru Constructive Affixture"),
            "txtTax15C" => Some("Advance Payment during the month"),
            "txtTax15D" => Some("Total Payments"),
            "txtTax16" => Some("Net Tax Payable/(Overpayment)/(Balance to be carried over)"),
            "txtTax17A" => Some("Surcharge"),
            "txtTax17B" => Some("Interest"),
            "txtTax17C" => Some("Compromise"),
            "txtTax17D" => Some("Total Penalties"),
            "txtTax18" => Some("Total Amount Payable/(Overpayment)/(Balance to be carried over)"),
            "txtTax19" => Some("Total Amount of Documentary Stamps Sold for the Month"),
            _ => None,
        },
        "2550Q" => match stem {
            "calendarNo1" | "fiscalNo1" => Some("For: Calendar / Fiscal"),
            "selectedMonthNo2" | "txtYearNo2" => Some("Year Ended (MM/YYYY)"),
            "OptQuarter1" | "OptQuarter2" | "OptQuarter3" | "OptQuarter4" => Some("Quarter"),
            "RtnPeriodFromNo4" | "RtnPeriodToNo4" => Some("Return Period (MM/DD/YYYY)"),
            "amendedReturnYesNo5" | "amendedReturnNo5" => Some("Amended Return?"),
            "OptShortPrd1" | "OptShortPrd2" => Some("Short Period Return?"),
            "txtTIN1" | "txtTIN2" | "txtTIN3" | "branchCode" => {
                Some("Taxpayer Identification Number (TIN)")
            }
            "txtRDOCode" => Some("RDO Code"),
            "taxpayerName" => Some("Taxpayer’s Name"),
            "taxpayerAddress" => Some("Registered Address"),
            "taxpayerZip" => Some("ZIP Code"),
            "taxpayerContactNumber" => Some("Contact Number"),
            "taxpayerEmailAddress" => Some("Email Address"),
            "taxPayerClassification1"
            | "taxPayerClassification2"
            | "taxPayerClassification3"
            | "taxPayerClassification4" => Some("Taxpayer Classification"),
            "internationalTreatyYn" | "specialRateYn" | "specifyInternationalTreaty" => {
                Some("Special Law or International Tax Treaty?")
            }
            "excessInputTax" => Some("Net VAT Payable/(Excess Input Tax)"),
            "creditableVat" => Some("Creditable VAT Withheld"),
            "advVatPayment" => Some("Advance VAT Payments"),
            "vatPaidReturn" => Some("VAT paid in return previously filed, if amended"),
            "addSpecifyNo19" | "otherCreditsNo19" => Some("Other Credits/Payment (Specify)"),
            "totalTaxCredits" => Some("Total Tax Credits/Payment"),
            "excessCredits" => Some("Tax Still Payable/(Excess Credits)"),
            "surcharge" => Some("Surcharge"),
            "interest" => Some("Interest"),
            "compromise" => Some("Compromise"),
            "penalties" => Some("Total Penalties"),
            "totalPayable" => Some("TOTAL AMOUNT PAYABLE/(Excess Credits)"),
            _ => None,
        },
        _ => None,
    }
}

fn heuristic_physical_label(key: &str) -> String {
    let stem = key.rsplit(':').next().unwrap_or(key);
    let mut label = stem
        .trim_start_matches("txt")
        .trim_start_matches("opt")
        .trim_start_matches("rb")
        .trim_start_matches("cb")
        .trim_start_matches("chk")
        .trim_start_matches("drp")
        .trim_start_matches("selected")
        .replace("TIN", "Taxpayer Identification Number")
        .replace("RDO", "RDO")
        .replace("ATC", "ATC")
        .replace("LineBus", "Line of Business")
        .replace("TaxpayerName", "Taxpayer/Withholding Agent’s Name")
        .replace("TelNum", "Contact Number")
        .replace("BranchCode", "Branch Code")
        .replace("ZipCode", "ZIP Code")
        .replace("AmendedRtn", "Amended Return")
        .replace("Amend", "Amended Return")
        .replace("Withheld", "Taxes Withheld")
        .replace("Quarter", "Quarter")
        .replace("Month", "Month")
        .replace("Year", "Year")
        .replace("Agency", "Drawee Bank/Agency")
        .replace("Number", "Reference Number")
        .replace("Date", "Date")
        .replace("Amount", "Amount")
        .replace("Particular", "Particulars")
        .replace("Tax", "Tax / Amount");
    let prefixes = [
        "No",
        "Tax",
        "Amount",
        "Agency",
        "Number",
        "Date",
        "Particular",
        "Item",
        "Line",
    ];
    for prefix in prefixes {
        if let Some(item) = physical_item_no(key) {
            label = label.replace(&format!("{prefix}{item}"), prefix);
        }
    }
    let mut out = String::new();
    let mut previous_was_lower = false;
    for ch in label.chars() {
        if matches!(ch, '_' | '-') {
            out.push(' ');
            previous_was_lower = false;
        } else if ch.is_ascii_uppercase() && previous_was_lower {
            out.push(' ');
            out.push(ch);
            previous_was_lower = false;
        } else {
            out.push(ch);
            previous_was_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    let cleaned = out.replace("  ", " ").trim().to_string();
    if cleaned.is_empty() {
        key.to_string()
    } else {
        cleaned
    }
}

fn is_wide_physical_field(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("taxpayername")
        || lower.contains("address")
        || lower.contains("email")
        || lower.contains("linebus")
}

fn physical_form_title(form_code: &str) -> &'static str {
    match form_code {
        "0619E" => "Monthly Remittance Form",
        "1601EQ" => "Quarterly Remittance Return",
        "1701Q" => "Quarterly Income Tax Return",
        "1702Q" => "Quarterly Income Tax Return",
        "2000" => "Documentary Stamp Tax Declaration/Return",
        "2550Q" => "Quarterly Value-Added Tax Return",
        _ => "Tax Return",
    }
}

fn physical_form_subtitle(form_code: &str) -> &'static str {
    match form_code {
        "0619E" => "Creditable Income Taxes Withheld (Expanded)",
        "1601EQ" => "Creditable Income Taxes Withheld (Expanded)",
        "1701Q" => "For Individuals, Estates and Trusts",
        "1702Q" => "For Corporations, Partnerships and Other Non-Individual Taxpayers",
        "2000" => "Documentary Stamp Tax",
        "2550Q" => "Value-Added Tax",
        _ => "BIR form data entry",
    }
}

fn physical_form_version(form_code: &str) -> &'static str {
    match form_code {
        "2550Q" => "April 2024 (ENCS)",
        "1601EQ" => "January 2019 (ENCS)",
        _ => "January 2018 (ENCS)",
    }
}

fn physical_computation_title(form_code: &str) -> &'static str {
    match form_code {
        "0619E" => "Part II – Tax Remittance",
        "2550Q" => "Part II – Total Tax Payable",
        _ => "Part II – Computation of Tax",
    }
}

fn physical_payment_title(form_code: &str) -> &'static str {
    match form_code {
        "2550Q" => "Part III – Details of Payment",
        _ => "Part III – Details of Payment",
    }
}

fn physical_schedule_title(form_code: &str) -> &'static str {
    match form_code {
        "1601EQ" => "Page 2 – ATC / Tax Remittance Schedule",
        "2550Q" => "Part V – Schedules",
        "1701Q" => "Schedules / Other Supporting Fields",
        "1702Q" => "Schedules / Other Supporting Fields",
        _ => "Schedules / Other Supporting Fields",
    }
}

fn update_top_level_value(
    set_form_input_text: WriteSignal<String>,
    section: &str,
    key: &str,
    value: Value,
) {
    let field_identity = format!("{section}:{key}");
    let view_state = capture_form_view_state(&field_identity);
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(obj) = root.get_mut(section).and_then(Value::as_object_mut) {
                obj.insert(key.to_string(), value);
                *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
            }
        }
    });
    preserve_form_view_after_update(view_state);
}

#[derive(Clone)]
struct FormViewState {
    field_key: String,
    scroll_x: f64,
    scroll_y: f64,
    selection_start: Option<u32>,
    selection_end: Option<u32>,
}

fn capture_form_view_state(field_key: &str) -> Option<FormViewState> {
    let window = web_sys::window()?;
    let active_input = window
        .document()
        .and_then(|document| document.active_element())
        .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok());
    Some(FormViewState {
        field_key: field_key.to_string(),
        scroll_x: window.scroll_x().unwrap_or_default(),
        scroll_y: window.scroll_y().unwrap_or_default(),
        selection_start: active_input
            .as_ref()
            .and_then(|input| input.selection_start().ok().flatten()),
        selection_end: active_input
            .as_ref()
            .and_then(|input| input.selection_end().ok().flatten()),
    })
}

fn restore_form_view_state(state: &FormViewState) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Some(document) = window.document() {
        let selector = format!(r#"[data-form-field="{}"]"#, state.field_key);
        if let Ok(Some(element)) = document.query_selector(&selector) {
            if let Some(html_element) = element.dyn_ref::<web_sys::HtmlElement>() {
                let _ = html_element.focus();
            }
            if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
                if let (Some(start), Some(end)) = (state.selection_start, state.selection_end) {
                    let _ = input.set_selection_range(start, end);
                }
            }
        }
    }
    window.scroll_to_with_x_and_y(state.scroll_x, state.scroll_y);
}

fn preserve_form_view_after_update(state: Option<FormViewState>) {
    let Some(state) = state else {
        return;
    };
    restore_form_view_state(&state);
    let Some(window) = web_sys::window() else {
        return;
    };
    let callback = Closure::<dyn FnMut()>::once(move || restore_form_view_state(&state));
    let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    callback.forget();
}

fn update_field_string(set_form_input_text: WriteSignal<String>, key: &str, value: String) {
    // HumanTaxForm currently rebuilds its PDF-like subtree when the JSON signal changes.
    // Preserve the active control and viewport so controlled numeric inputs do not throw
    // the operator back to the top of a long form after every keystroke.
    let view_state = capture_form_view_state(key);
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                fields.insert(key.to_string(), Value::String(value));
                recalculate_1701q_payload(&mut root);
                *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
            }
        }
    });
    preserve_form_view_after_update(view_state);
}

fn update_period_component(
    set_form_input_text: WriteSignal<String>,
    key: &str,
    part: &str,
    sync_return_period: bool,
    value: String,
) {
    let view_state = capture_form_view_state(key);
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            let normalized = if part == "month" {
                format!("{:0>2}", value.trim())
            } else {
                value.trim().to_string()
            };
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                fields.insert(key.to_string(), Value::String(normalized.clone()));
            }
            if sync_return_period {
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
            }
            *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
        }
    });
    preserve_form_view_after_update(view_state);
}

fn update_pair_fields(
    set_form_input_text: WriteSignal<String>,
    yes_key: &str,
    no_key: &str,
    yes_selected: bool,
) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                fields.insert(yes_key.to_string(), Value::String(yes_selected.to_string()));
                fields.insert(
                    no_key.to_string(),
                    Value::String((!yes_selected).to_string()),
                );
            }
            let marker = format!(
                "{}{}",
                yes_key.to_ascii_lowercase(),
                no_key.to_ascii_lowercase()
            );
            if marker.contains("amend") {
                if let Some(ret) = root.get_mut("return").and_then(Value::as_object_mut) {
                    ret.insert("is_amended".to_string(), Value::Bool(yes_selected));
                }
            }
            *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
        }
    });
}

fn update_choice_fields(
    set_form_input_text: WriteSignal<String>,
    keys: Vec<&str>,
    selected_key: &str,
) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                for key in &keys {
                    fields.insert(
                        (*key).to_string(),
                        Value::String((*key == selected_key).to_string()),
                    );
                }
                let osd_inputs = match selected_key {
                    "frm1701:optMethodOfDeduction23:_2" => Some(["frm1701q:txt37A", "frm1701q:txt38C"]),
                    "frm1701:optMethodOfDeduction24:_2" => Some(["frm1701q:txt37B", "frm1701q:txt38D"]),
                    _ => None,
                };
                if let Some(osd_inputs) = osd_inputs {
                    for key in osd_inputs {
                        fields.insert(key.to_string(), Value::String("0.00".to_string()));
                    }
                }
            }
            if let Some(quarter) = selected_quarter_from_key(selected_key) {
                if let Some(period) = root
                    .get_mut("return")
                    .and_then(Value::as_object_mut)
                    .and_then(|ret| ret.get_mut("period"))
                    .and_then(Value::as_object_mut)
                {
                    period.insert("quarter".to_string(), Value::Number(quarter.into()));
                }
            }
            recalculate_1701q_payload(&mut root);
            *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
        }
    });
}

fn selected_quarter_from_key(key: &str) -> Option<i64> {
    let lower = key.to_ascii_lowercase();
    if !lower.contains("quarter") && !lower.contains("optquarter") {
        return None;
    }
    key.chars()
        .rev()
        .find(|ch| ch.is_ascii_digit())
        .and_then(|ch| ch.to_digit(10))
        .map(i64::from)
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

fn render_1601c_period_box(
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let month = field_value(input, "txtMonth");
    let year = field_value(input, "txtYear");
    view! {
        <label class="bir-box">"1 For the Month (MM/YYYY)"
            <div class="split-inputs">
                <input data-form-field="txtMonth" aria-label="Month" prop:value=month prop:readonly=locked on:input=move |ev| update_1601c_period(set_form_input_text, "month", event_target_value(&ev)) />
                <span>"/"</span>
                <input data-form-field="txtYear" aria-label="Year" prop:value=year prop:readonly=locked on:input=move |ev| update_1601c_period(set_form_input_text, "year", event_target_value(&ev)) />
            </div>
        </label>
    }.into_view()
}

fn render_1601c_tin_box(
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let tin1 = field_value(input, "txtTIN1");
    let tin2 = field_value(input, "txtTIN2");
    let tin3 = field_value(input, "txtTIN3");
    let branch = field_value(input, "txtBranchCode");
    view! {
        <label class="bir-box span-2">"6 Taxpayer Identification Number (TIN)"
            <div class="split-inputs tin-inputs">
                <input data-form-field="txtTIN1" aria-label="TIN first block" prop:value=tin1 prop:readonly=locked on:input=move |ev| update_1601c_tin(set_form_input_text, "txtTIN1", event_target_value(&ev)) />
                <span>"/"</span>
                <input data-form-field="txtTIN2" aria-label="TIN second block" prop:value=tin2 prop:readonly=locked on:input=move |ev| update_1601c_tin(set_form_input_text, "txtTIN2", event_target_value(&ev)) />
                <span>"/"</span>
                <input data-form-field="txtTIN3" aria-label="TIN third block" prop:value=tin3 prop:readonly=locked on:input=move |ev| update_1601c_tin(set_form_input_text, "txtTIN3", event_target_value(&ev)) />
                <span>"/"</span>
                <input data-form-field="txtBranchCode" aria-label="Branch code" prop:value=branch prop:readonly=locked on:input=move |ev| update_1601c_tin(set_form_input_text, "txtBranchCode", event_target_value(&ev)) />
            </div>
        </label>
    }.into_view()
}

fn render_1601c_profile_email_box(
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let value = input
        .get("profile")
        .and_then(|profile| profile.get("email"))
        .map(value_to_form_string)
        .unwrap_or_default();
    view! {
        <label class="bir-box span-2">"12 Email Address"
            <input data-form-field="profile:email" prop:value=value prop:readonly=locked on:input=move |ev| update_top_level_value(set_form_input_text, "profile", "email", Value::String(event_target_value(&ev))) />
        </label>
    }.into_view()
}

fn render_1601c_field_box(
    item: &'static str,
    label: &'static str,
    key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let value = field_value(input, key);
    view! {
        <label class="bir-box">{format!("{item} {label}")}
            <input data-form-field=key prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, key, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_1601c_wide_field_box(
    item: &'static str,
    label: &'static str,
    key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let value = field_value(input, key);
    view! {
        <label class="bir-box span-2">{format!("{item} {label}")}
            <input data-form-field=key prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, key, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_1601c_pair_box(
    item: &'static str,
    label: &'static str,
    yes_key: &'static str,
    no_key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
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

fn render_1601c_amount_row(
    item: &'static str,
    label: &'static str,
    key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let value = field_value(input, key);
    view! {
        <label class="bir-row">
            <span class="item-no">{item}</span><span class="item-label">{label}</span>
            <input data-form-field=key class="amount-input" prop:value=value prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, key, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_1601c_amount_row_with_specify(
    item: &'static str,
    label: &'static str,
    specify_key: &'static str,
    amount_key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let specify = field_value(input, specify_key);
    let amount = field_value(input, amount_key);
    view! {
        <label class="bir-row specify-row">
            <span class="item-no">{item}</span><span class="item-label">{label}</span>
            <input data-form-field=specify_key class="specify-input" placeholder="specify" prop:value=specify prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, specify_key, event_target_value(&ev)) />
            <input data-form-field=amount_key class="amount-input" prop:value=amount prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, amount_key, event_target_value(&ev)) />
        </label>
    }.into_view()
}

fn render_1601c_payment_row(
    item: &'static str,
    label: &'static str,
    agency_key: Option<&'static str>,
    number_key: &'static str,
    date_key: &'static str,
    amount_key: &'static str,
    input: &Value,
    set_form_input_text: WriteSignal<String>,
    locked: bool,
) -> View {
    let agency = agency_key
        .map(|key| field_value(input, key))
        .unwrap_or_default();
    let number = field_value(input, number_key);
    let date = field_value(input, date_key);
    let amount = field_value(input, amount_key);
    view! {
        <div class="payment-row">
            <strong>{format!("{item} {label}")}</strong>
            <input data-form-field=agency_key.unwrap_or("") placeholder="Drawee Bank/Agency" prop:value=agency prop:readonly=locked on:input=move |ev| if let Some(key) = agency_key { update_field_string(set_form_input_text, key, event_target_value(&ev)) } />
            <input data-form-field=number_key placeholder="Number" prop:value=number prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, number_key, event_target_value(&ev)) />
            <input data-form-field=date_key placeholder="Date (MM/DD/YYYY)" prop:value=date prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, date_key, event_target_value(&ev)) />
            <input data-form-field=amount_key class="amount-input" placeholder="Amount" prop:value=amount prop:readonly=locked on:input=move |ev| update_field_string(set_form_input_text, amount_key, event_target_value(&ev)) />
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
    let field_key = if part == "month" {
        "txtMonth"
    } else {
        "txtYear"
    };
    let view_state = capture_form_view_state(field_key);
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            let normalized = if part == "month" {
                format!("{:0>2}", value.trim())
            } else {
                value.trim().to_string()
            };
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
    preserve_form_view_after_update(view_state);
}

fn update_1601c_tin(set_form_input_text: WriteSignal<String>, key: &str, value: String) {
    let view_state = capture_form_view_state(key);
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                fields.insert(key.to_string(), Value::String(value));
                let tin1 = fields
                    .get("txtTIN1")
                    .map(value_to_form_string)
                    .unwrap_or_default();
                let tin2 = fields
                    .get("txtTIN2")
                    .map(value_to_form_string)
                    .unwrap_or_default();
                let tin3 = fields
                    .get("txtTIN3")
                    .map(value_to_form_string)
                    .unwrap_or_default();
                let branch = fields
                    .get("txtBranchCode")
                    .map(value_to_form_string)
                    .unwrap_or_default();
                fields.insert("txtPg2TIN1".to_string(), Value::String(tin1.clone()));
                fields.insert("txtPg2TIN2".to_string(), Value::String(tin2.clone()));
                fields.insert("txtPg2TIN3".to_string(), Value::String(tin3.clone()));
                fields.insert(
                    "txtPg2BranchCode".to_string(),
                    Value::String(branch.clone()),
                );
                if let Some(profile) = root.get_mut("profile").and_then(Value::as_object_mut) {
                    profile.insert(
                        "tin".to_string(),
                        Value::String(format!("{tin1}-{tin2}-{tin3}-{branch}")),
                    );
                }
                *text = serde_json::to_string_pretty(&root).unwrap_or_else(|_| text.clone());
            }
        }
    });
    preserve_form_view_after_update(view_state);
}

fn update_checkbox_pair(
    set_form_input_text: WriteSignal<String>,
    yes_key: &str,
    no_key: &str,
    yes_selected: bool,
) {
    set_form_input_text.update(|text| {
        if let Ok(mut root) = serde_json::from_str::<Value>(text) {
            if let Some(fields) = root.get_mut("fields").and_then(Value::as_object_mut) {
                fields.insert(yes_key.to_string(), Value::String(yes_selected.to_string()));
                fields.insert(
                    no_key.to_string(),
                    Value::String((!yes_selected).to_string()),
                );
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
fn LockScreen<F>(
    pin: ReadSignal<String>,
    set_pin: WriteSignal<String>,
    unlock_app: F,
) -> impl IntoView
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
fn Settings<T, S, L>(
    theme: ReadSignal<String>,
    set_theme_preference: T,
    submission_mode: ReadSignal<String>,
    set_submission_mode_preference: S,
    pin: ReadSignal<String>,
    set_pin: WriteSignal<String>,
    lock_now: L,
) -> impl IntoView
where
    T: Fn(&'static str) + Copy + 'static,
    S: Fn(&'static str) + Copy + 'static,
    L: Fn() + Copy + 'static,
{
    view! {
        <Panel title="Settings">
            <p>"Dry-run remains the default. Live submission is gated by validation, final-copy confirmation, a live-mode confirmation dialog, and receipt matching."</p>
            <h3>"Submission mode"</h3>
            <div class="theme-controls">
                <button class=move || if submission_mode.get() == "dry_run" { "active" } else { "" } on:click=move |_| set_submission_mode_preference("dry_run")>"Dry run only"</button>
                <button class=move || if submission_mode.get() == "live" { "active danger" } else { "danger" } on:click=move |_| set_submission_mode_preference("live")>"Live submit to BIR"</button>
            </div>
            <p class="muted">"Live mode uses the packaged production SFTP transport. Keep dry run selected unless the taxpayer has authorized final filing."</p>
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

fn normalize_submission_mode(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().replace('-', "_").as_str() {
        "dry_run" | "dryrun" | "dry" => Some("dry_run"),
        "live" => Some("live"),
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

fn personalize_form_input(
    form_code: &str,
    sample_input: &str,
    profile: &TaxpayerProfileResponse,
) -> String {
    let mut input: Value = match serde_json::from_str(sample_input) {
        Ok(value) => value,
        Err(_) => return sample_input.to_string(),
    };

    if let Some(profile_json) = input.get_mut("profile").and_then(Value::as_object_mut) {
        profile_json.insert("profile_id".to_string(), Value::String(profile.profile_id.clone()));
        profile_json.insert("tin".to_string(), Value::String(profile.tin.clone()));
        profile_json.insert("email".to_string(), Value::String(profile.email.clone()));
    }

    if form_code == "1701Q" {
        let tin_digits: String = profile.tin.chars().filter(char::is_ascii_digit).collect();
        let tin_part = |start: usize, end: usize| {
            tin_digits
                .get(start..end.min(tin_digits.len()))
                .unwrap_or_default()
                .to_string()
        };
        if let Some(fields) = input.get_mut("fields").and_then(Value::as_object_mut) {
            let mut set = |key: &str, value: String| {
                fields.insert(key.to_string(), Value::String(value));
            };
            set("frm1701q:txt5TIN1", tin_part(0, 3));
            set("frm1701q:txt5TIN2", tin_part(3, 6));
            set("frm1701q:txt5TIN3", tin_part(6, 9));
            set("frm1701q:txt5BranchCode", tin_part(9, 14));
            set(
                "frm1701q:txt5RDOCode",
                profile.rdo_code.clone().unwrap_or_default(),
            );
            set(
                "frm1701q:txtTaxPayername",
                encode_bir_text(&profile.taxpayer_name),
            );
            set(
                "frm1701q:txt11Address",
                encode_bir_text(profile.registered_address.as_deref().unwrap_or_default()),
            );
            set(
                "frm1701q:txt14zip",
                profile.zip_code.clone().unwrap_or_default(),
            );
            set("txtEmail", profile.email.clone());
        }
    }

    if form_code == "1701Q" {
        recalculate_1701q_payload(&mut input);
    }
    serde_json::to_string_pretty(&input).unwrap_or_else(|_| sample_input.to_string())
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

#[cfg(test)]
mod integration_tests {
    use super::*;

    fn set_field(root: &mut Value, key: &str, value: &str) {
        root.get_mut("fields")
            .and_then(Value::as_object_mut)
            .expect("fixture fields")
            .insert(key.to_string(), Value::String(value.to_string()));
    }

    fn field(root: &Value, key: &str) -> String {
        root.get("fields")
            .and_then(Value::as_object)
            .and_then(|fields| fields.get(key))
            .map(value_to_form_string)
            .expect("calculated field")
    }

    #[test]
    fn box_key_crosswalk_covers_part_three_and_every_schedule_amount_column() {
        for item in 26..=30 {
            assert!(box_field_key(item, 'A').is_some(), "missing Box {item}A key");
            assert!(box_field_key(item, 'B').is_some(), "missing Box {item}B key");
        }
        for item in 36..=68 {
            assert!(box_field_key(item, 'A').is_some(), "missing Box {item}A key");
            assert!(box_field_key(item, 'B').is_some(), "missing Box {item}B key");
        }
    }

    #[test]
    fn profile_personalization_preserves_bir_encoding_and_populates_box_12() {
        let profile = TaxpayerProfileResponse {
            profile_id: "profile-test".to_string(),
            taxpayer_name: "JUAN DELA CRUZ".to_string(),
            tin: "12345678900000".to_string(),
            email: "juan@example.test".to_string(),
            rdo_code: Some("001".to_string()),
            registered_address: Some("1 TEST STREET".to_string()),
            zip_code: Some("1000".to_string()),
            created_unix_seconds: 0,
            updated_unix_seconds: 0,
        };
        let personalized = personalize_form_input("1701Q", FORM_1701Q, &profile);
        let root: Value = serde_json::from_str(&personalized).expect("personalized payload");
        assert_eq!(field(&root, "frm1701q:txtTaxPayername"), "JUAN%20DELA%20CRUZ");
        assert_eq!(
            display_1701q_field_value(
                "frm1701q:txtTaxPayername",
                field(&root, "frm1701q:txtTaxPayername"),
            ),
            "JUAN DELA CRUZ"
        );
        assert_eq!(field(&root, "frm1701q:txt11Address"), "1%20TEST%20STREET");
        assert_eq!(field(&root, "txtEmail"), "juan@example.test");
        assert_eq!(field(&root, "frm1701q:txt5TIN1"), "123");
        assert_eq!(field(&root, "frm1701q:txt5BranchCode"), "00000");
    }

    #[test]
    fn atc_switch_clears_only_the_selected_filers_inactive_schedule() {
        let mut root: Value = serde_json::from_str(FORM_1701Q).expect("1701Q fixture");
        let fields = root
            .get_mut("fields")
            .and_then(Value::as_object_mut)
            .expect("fixture fields");
        fields.insert("frm1701q:txt36A".to_string(), Value::String("100.00".to_string()));
        fields.insert("frm1701q:txt40A".to_string(), Value::String("200.00".to_string()));
        fields.insert("frm1701q:txt36B".to_string(), Value::String("300.00".to_string()));
        apply_1701q_atc_fields(fields, false, "ui1701q:taxpayer_atc_ii015");
        assert_eq!(fields.get("frm1701q:txt36A").map(value_to_form_string).as_deref(), Some("0.00"));
        assert_eq!(fields.get("frm1701q:txt40A").map(value_to_form_string).as_deref(), Some("200.00"));
        assert_eq!(fields.get("frm1701q:txt36B").map(value_to_form_string).as_deref(), Some("300.00"));

        fields.insert("frm1701q:txt40B".to_string(), Value::String("400.00".to_string()));
        apply_1701q_atc_fields(fields, true, "ui1701q:spouse_atc_ii012");
        assert_eq!(fields.get("frm1701q:txt40B").map(value_to_form_string).as_deref(), Some("0.00"));
        assert_eq!(fields.get("frm1701q:txt40A").map(value_to_form_string).as_deref(), Some("200.00"));
    }

    #[test]
    fn payload_recalculation_updates_both_columns_part_three_and_aggregate() {
        let mut root: Value = serde_json::from_str(FORM_1701Q).expect("1701Q fixture");

        set_field(&mut root, "frm1701q:txtYear", "2026");
        set_field(&mut root, "frm1701q:txt36A", "1,000,000.00");
        set_field(&mut root, "frm1701q:txt37A", "0.00");
        set_field(&mut root, "frm1701q:txt38I", "0.00");
        set_field(&mut root, "frm1701q:txt38K", "0.00");
        set_field(&mut root, "frm1701q:txt38M", "0.00");
        set_field(&mut root, "frm1701:optMethodOfDeduction23:_1", "false");
        set_field(&mut root, "frm1701:optMethodOfDeduction23:_2", "true");

        for key in [
            "ui1701q:spouse_atc_ii012",
            "ui1701q:spouse_atc_ii014",
            "ui1701q:spouse_atc_ii013",
            "ui1701q:spouse_atc_ii017",
            "ui1701q:spouse_atc_ii016",
        ] {
            set_field(&mut root, key, "false");
        }
        set_field(&mut root, "ui1701q:spouse_atc_ii015", "true");
        set_field(&mut root, "frm1701q:txt40B", "500,000.00");
        set_field(&mut root, "frm1701q:txt40D", "0.00");
        set_field(&mut root, "frm1701q:txt40H", "0.00");
        set_field(&mut root, "ui1701q:txt52B", "250,000.00");
        set_field(&mut root, "ui1701q:txt55B", "5,000.00");
        set_field(&mut root, "ui1701q:txt64B", "2,000.00");

        recalculate_1701q_payload(&mut root);

        assert_eq!(field(&root, "frm1701q:txt38E"), "400,000.00");
        assert_eq!(field(&root, "frm1701q:txt38G"), "600,000.00");
        assert_eq!(field(&root, "frm1701q:txt39A"), "600,000.00");
        assert_eq!(field(&root, "ui1701q:txt46A"), "62,500.00");
        assert_eq!(field(&root, "frm1701q:txt26A"), "62,500.00");

        assert_eq!(field(&root, "frm1701q:txt40F"), "500,000.00");
        assert_eq!(field(&root, "frm1701q:txt41B"), "500,000.00");
        assert_eq!(field(&root, "ui1701q:txt53B"), "250,000.00");
        assert_eq!(field(&root, "ui1701q:txt54B"), "20,000.00");
        assert_eq!(field(&root, "ui1701q:txt62B"), "5,000.00");
        assert_eq!(field(&root, "ui1701q:txt63B"), "15,000.00");
        assert_eq!(field(&root, "ui1701q:txt67B"), "2,000.00");
        assert_eq!(field(&root, "ui1701q:txt68B"), "17,000.00");
        assert_eq!(field(&root, "frm1701q:txt30B"), "17,000.00");
        assert_eq!(field(&root, "frm1701q:txt31A"), "79,500.00");
    }
}
