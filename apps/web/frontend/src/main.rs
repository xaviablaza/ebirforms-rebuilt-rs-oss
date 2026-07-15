use leptos::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen(module = "/src/api.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn api(path: &str, method: &str, body: JsValue) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Default, Deserialize)]
struct Session {
    email: String,
    role: String,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct Intake {
    id: i64,
    owner_email: String,
    form_code: String,
    payload: Value,
    revision: i64,
    state: String,
    workflow_status: Option<String>,
    reference: Option<String>,
}

async fn request<T: for<'de> Deserialize<'de>>(
    path: &str,
    method: &str,
    body: Value,
) -> Result<T, String> {
    let value = api(path, method, serde_wasm_bindgen::to_value(&body).unwrap())
        .await
        .map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|e| e.to_string())
}
fn js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(&value, &JsValue::from_str("message"))
                .ok()?
                .as_string()
        })
        .unwrap_or_else(|| "Request failed".into())
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (session, set_session) = create_signal(None::<Session>);
    let (message, set_message) = create_signal(String::new());
    spawn_local(async move {
        if let Ok(me) = request::<Session>("/auth/me", "GET", Value::Null).await {
            set_session.set(Some(me));
        }
    });
    view! {
      <header><a class="brand" href="/">"eBIRForms assisted filing"</a><span>"Secure intake · unofficial"</span></header>
      <main>
        {move || match session.get() {
          None => view! { <Login set_session=set_session message=message set_message=set_message/> }.into_view(),
          Some(me) if me.role == "operator" => view! { <Operator me=me set_session=set_session message=message set_message=set_message/> }.into_view(),
          Some(me) => view! { <Customer me=me set_session=set_session message=message set_message=set_message/> }.into_view(),
        }}
      </main>
    }
}

#[component]
fn Login(
    set_session: WriteSignal<Option<Session>>,
    message: ReadSignal<String>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let (email, set_email) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let submit = move |_| {
        let body = json!({"email":email.get(),"password":password.get()});
        spawn_local(async move {
            match request::<Session>("/auth/login", "POST", body).await {
                Ok(me) => {
                    set_message.set(String::new());
                    set_session.set(Some(me))
                }
                Err(e) => set_message.set(e),
            }
        })
    };
    view! { <section class="card login"><p class="eyebrow">"Private customer portal"</p><h1>"Continue your assisted filing"</h1><p>"Your account is created by our team before your guided call."</p><label>"Email"<input type="email" on:input=move|e|set_email.set(event_target_value(&e))/></label><label>"Password"<input type="password" on:input=move|e|set_password.set(event_target_value(&e))/></label><button on:click=submit>"Sign in"</button><p class="error">{move||message.get()}</p></section> }
}

fn logout(set_session: WriteSignal<Option<Session>>, set_message: WriteSignal<String>) {
    spawn_local(async move {
        let _: Result<Value, _> = request("/auth/logout", "POST", json!({})).await;
        set_session.set(None);
        set_message.set(String::new());
    });
}

#[component]
fn Customer(
    me: Session,
    set_session: WriteSignal<Option<Session>>,
    message: ReadSignal<String>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let (items, set_items) = create_signal(Vec::<Intake>::new());
    let (selected, set_selected) = create_signal(None::<Intake>);
    let (form_code, set_form_code) = create_signal("1701Q".to_string());
    let refresh = move || {
        spawn_local(async move {
            match request::<Vec<Intake>>("/intakes", "GET", Value::Null).await {
                Ok(v) => set_items.set(v),
                Err(e) => set_message.set(e),
            }
        })
    };
    refresh();
    let create = move |_| {
        let code = form_code.get();
        spawn_local(async move {
            match request::<Value>("/intakes", "POST", json!({"form_code":code})).await {
                Ok(_) => refresh(),
                Err(e) => set_message.set(e),
            }
        })
    };
    view! { <div class="toolbar"><div><p class="eyebrow">"Customer intake"</p><h1>{format!("Welcome, {}",me.email)}</h1></div><button class="secondary" on:click=move |_|logout(set_session,set_message)>"Sign out"</button></div>
    <p class="notice">"This portal collects your information for review. It does not file directly with BIR. Our team will file through official eBIRForms and send the official receipt afterward."</p>
    <div class="grid"><section class="card"><h2>"Your returns"</h2><div class="new"><select on:change=move|e|set_form_code.set(event_target_value(&e))><option>"1701Q"</option><option>"1702Q"</option></select><button on:click=create>"Start intake"</button></div><ul class="items">{move||items.get().into_iter().map(|i|{let copy=i.clone();view!{<li><button class="item" on:click=move |_|set_selected.set(Some(copy.clone()))><strong>{i.form_code}</strong><span>{i.reference.unwrap_or_else(||"Draft".into())}</span></button></li>}}).collect_view()}</ul></section>
    <section class="card editor">{move||selected.get().map(|i|view!{<IntakeEditor intake=i set_selected=set_selected set_items=set_items set_message=set_message/>}).unwrap_or_else(||view!{<div class="empty"><h2>"Choose or start an intake"</h2><p>"During your guided call, complete the return data and save as you go."</p></div>}.into_view())}</section></div><p class="error">{move||message.get()}</p> }
}

#[derive(Clone, Copy)]
struct GuidedField {
    label: &'static str,
    path: &'static str,
    input_type: &'static str,
    hint: &'static str,
}

const COMMON_FIELDS: &[GuidedField] = &[
    GuidedField {
        label: "Taxpayer identification number (TIN)",
        path: "profile.tin",
        input_type: "text",
        hint: "Enter the 9-digit TIN and 5-digit branch code",
    },
    GuidedField {
        label: "Tax year",
        path: "period.year",
        input_type: "number",
        hint: "Example: 2026",
    },
];
const FIELDS_1701Q: &[GuidedField] = &[
    GuidedField {
        label: "Registered taxpayer name",
        path: "field:frm1701q:txtTaxPayername",
        input_type: "text",
        hint: "As shown on the BIR registration",
    },
    GuidedField {
        label: "RDO code",
        path: "field:frm1701q:txt5RDOCode",
        input_type: "text",
        hint: "Three-digit Revenue District Office code",
    },
    GuidedField {
        label: "Registered address",
        path: "field:frm1701q:txt11Address",
        input_type: "text",
        hint: "Complete registered address",
    },
    GuidedField {
        label: "ZIP code",
        path: "field:frm1701q:txt14zip",
        input_type: "text",
        hint: "Four digits",
    },
    GuidedField {
        label: "Telephone number",
        path: "field:frm1701q:txt15Telno",
        input_type: "tel",
        hint: "Optional",
    },
    GuidedField {
        label: "Birth month",
        path: "field:frm1701q:txt13BirthMonth",
        input_type: "number",
        hint: "1 to 12",
    },
    GuidedField {
        label: "Birth day",
        path: "field:frm1701q:txt13BirthDay",
        input_type: "number",
        hint: "1 to 31",
    },
    GuidedField {
        label: "Birth year",
        path: "field:frm1701q:txt13BirthYear",
        input_type: "number",
        hint: "Four digits",
    },
    GuidedField {
        label: "Citizenship",
        path: "field:ui1701q:taxpayer_citizenship",
        input_type: "text",
        hint: "Example: Filipino",
    },
    GuidedField {
        label: "Line of business or profession",
        path: "field:frm1701q:txt19",
        input_type: "text",
        hint: "Primary trade or profession",
    },
    GuidedField {
        label: "Sales, revenues, receipts or fees",
        path: "field:frm1701q:txt36A",
        input_type: "number",
        hint: "Amount for this quarter",
    },
    GuidedField {
        label: "Itemized deductions",
        path: "field:frm1701q:txt38C",
        input_type: "number",
        hint: "Enter 0 if not used",
    },
    GuidedField {
        label: "Optional standard deduction",
        path: "field:frm1701q:txt38E",
        input_type: "number",
        hint: "Enter 0 if not used",
    },
    GuidedField {
        label: "Taxable income from previous quarters",
        path: "field:frm1701q:txt38I",
        input_type: "number",
        hint: "Year-to-date prior quarter amount",
    },
    GuidedField {
        label: "Other non-operating income",
        path: "field:frm1701q:txt38K",
        input_type: "number",
        hint: "Enter 0 if none",
    },
    GuidedField {
        label: "Prior-year excess credits",
        path: "field:ui1701q:txt55A",
        input_type: "number",
        hint: "Enter 0 if none",
    },
    GuidedField {
        label: "Previous-quarter tax payments",
        path: "field:ui1701q:txt56A",
        input_type: "number",
        hint: "Enter 0 if none",
    },
    GuidedField {
        label: "Creditable tax withheld this quarter",
        path: "field:ui1701q:txt58A",
        input_type: "number",
        hint: "From BIR Form 2307",
    },
];
const FIELDS_1702Q: &[GuidedField] = &[
    GuidedField {
        label: "Registered corporate name",
        path: "field:txtTaxpayerName",
        input_type: "text",
        hint: "As shown on the BIR registration",
    },
    GuidedField {
        label: "RDO code",
        path: "field:txtRDOCode",
        input_type: "text",
        hint: "Three-digit Revenue District Office code",
    },
    GuidedField {
        label: "Registered address",
        path: "field:txtAddress",
        input_type: "text",
        hint: "Complete registered address",
    },
    GuidedField {
        label: "ZIP code",
        path: "field:txtZipCode",
        input_type: "text",
        hint: "Four digits",
    },
    GuidedField {
        label: "Telephone number",
        path: "field:txtTelNum",
        input_type: "tel",
        hint: "Optional",
    },
    GuidedField {
        label: "Alphanumeric tax code (ATC)",
        path: "field:txtATC",
        input_type: "text",
        hint: "Confirm with the filing adviser",
    },
    GuidedField {
        label: "Gross sales or receipts",
        path: "field:sched1_txtSales1",
        input_type: "number",
        hint: "Amount for this quarter",
    },
    GuidedField {
        label: "Cost of sales or services",
        path: "field:sched1_txtCost2",
        input_type: "number",
        hint: "Enter 0 if none",
    },
    GuidedField {
        label: "Other income",
        path: "field:sched1_txtOtherIncome4",
        input_type: "number",
        hint: "Enter 0 if none",
    },
    GuidedField {
        label: "Allowable deductions",
        path: "field:sched1_txtDeductions6",
        input_type: "number",
        hint: "Enter 0 if none",
    },
    GuidedField {
        label: "Taxable income from previous quarters",
        path: "field:sched1_txtPrevious8",
        input_type: "number",
        hint: "Year-to-date prior quarter amount",
    },
    GuidedField {
        label: "Prior-year excess credits",
        path: "field:sched4_txtPriorYearCredits1",
        input_type: "number",
        hint: "Enter 0 if none",
    },
    GuidedField {
        label: "Previous-quarter income tax payments",
        path: "field:sched4_txtPreviousPayments2",
        input_type: "number",
        hint: "Enter 0 if none",
    },
    GuidedField {
        label: "Creditable tax withheld this quarter",
        path: "field:sched4_txtCwtCurrent5",
        input_type: "number",
        hint: "Enter 0 if none",
    },
];

fn guided_value(payload: &Value, path: &str) -> String {
    if let Some(key) = path.strip_prefix("field:") {
        payload
            .get("fields")
            .and_then(|v| v.get(key))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into()
    } else if path == "profile.tin" {
        payload
            .pointer("/profile/tin")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into()
    } else {
        payload
            .pointer("/return/period/year")
            .and_then(Value::as_i64)
            .filter(|v| *v > 0)
            .map(|v| v.to_string())
            .unwrap_or_default()
    }
}
fn set_guided_value(payload: &mut Value, path: &str, value: String) {
    if let Some(key) = path.strip_prefix("field:") {
        if let Some(fields) = payload.get_mut("fields").and_then(Value::as_object_mut) {
            fields.insert(key.into(), Value::String(value));
        }
    } else if path == "profile.tin" {
        payload["profile"]["tin"] = Value::String(value)
    } else {
        payload["return"]["period"]["year"] =
            Value::Number(value.parse::<i64>().unwrap_or_default().into())
    }
}

async fn save_until_clean(
    id: i64,
    payload: RwSignal<Value>,
    revision: RwSignal<i64>,
    generation: RwSignal<u64>,
    saving: RwSignal<bool>,
) -> Result<(), String> {
    while saving.get_untracked() {
        gloo_timers::future::TimeoutFuture::new(30).await;
    }
    saving.set(true);
    loop {
        let current_generation = generation.get_untracked();
        let body = json!({"payload":payload.get_untracked(),"revision":revision.get_untracked()});
        match request::<Value>(&format!("/intakes/{id}"), "PATCH", body).await {
            Ok(value) => {
                if let Some(next) = value["revision"].as_i64() {
                    revision.set(next)
                }
            }
            Err(error) => {
                saving.set(false);
                return Err(error);
            }
        }
        if generation.get_untracked() == current_generation {
            break;
        }
    }
    saving.set(false);
    Ok(())
}

#[component]
fn GuidedInput(
    field: GuidedField,
    payload: RwSignal<Value>,
    generation: RwSignal<u64>,
    revision: RwSignal<i64>,
    saving: RwSignal<bool>,
    submitting: RwSignal<bool>,
    id: i64,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let update = move |event| {
        let value = event_target_value(&event);
        payload.update(|data| set_guided_value(data, field.path, value));
        generation.update(|value| *value += 1);
        let expected = generation.get_untracked();
        spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(650).await;
            if generation.get_untracked() == expected {
                if let Err(error) =
                    save_until_clean(id, payload, revision, generation, saving).await
                {
                    set_message.set(error)
                }
            }
        })
    };
    view! {<label class="guided-field"><span>{field.label}</span><input type=field.input_type disabled=move||submitting.get() prop:value=move||guided_value(&payload.get(),field.path) on:input=update/><small>{field.hint}</small></label>}
}

#[component]
fn IntakeEditor(
    intake: Intake,
    set_selected: WriteSignal<Option<Intake>>,
    set_items: WriteSignal<Vec<Intake>>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let payload = create_rw_signal(intake.payload.clone());
    let revision = create_rw_signal(intake.revision);
    let generation = create_rw_signal(0_u64);
    let saving = create_rw_signal(false);
    let submitting = create_rw_signal(false);
    let id = intake.id;
    let locked = intake.state != "draft";
    let fields = if intake.form_code == "1701Q" {
        FIELDS_1701Q
    } else {
        FIELDS_1702Q
    };
    let set_quarter = move |event| {
        let value = event_target_value(&event)
            .parse::<i64>()
            .unwrap_or_default();
        payload.update(|data| data["return"]["period"]["quarter"] = Value::Number(value.into()));
        generation.update(|value| *value += 1);
        let expected = generation.get_untracked();
        spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(650).await;
            if generation.get_untracked() == expected {
                if let Err(error) =
                    save_until_clean(id, payload, revision, generation, saving).await
                {
                    set_message.set(error)
                }
            }
        });
    };
    let save = move |_| {
        spawn_local(async move {
            match save_until_clean(id, payload, revision, generation, saving).await {
                Ok(()) => set_message.set("Draft saved.".into()),
                Err(error) => set_message.set(error),
            }
        })
    };
    let submit = move |_| {
        spawn_local(async move {
            submitting.set(true);
            if let Err(error) = save_until_clean(id, payload, revision, generation, saving).await {
                submitting.set(false);
                set_message.set(error);
                return;
            }
            match request::<Value>(&format!("/intakes/{id}/submit"), "POST", json!({})).await {
                Ok(value) => {
                    set_message.set(
                        value["message"]
                            .as_str()
                            .unwrap_or("Information received.")
                            .into(),
                    );
                    set_selected.set(None);
                    if let Ok(items) = request::<Vec<Intake>>("/intakes", "GET", Value::Null).await
                    {
                        set_items.set(items)
                    }
                }
                Err(error) => {
                    submitting.set(false);
                    set_message.set(error)
                }
            }
        })
    };
    view! {<div><p class="eyebrow">{format!("{} guided intake",intake.form_code)}</p><h2>{intake.reference.clone().unwrap_or_else(||"Draft return".into())}</h2>{if locked{view!{<div class="success"><h3>"Information received"</h3><p>"Our team will review and file this return. The official receipt will follow after filing."</p></div>}.into_view()}else{view!{<><p>"Complete each section with your filing adviser. Changes are saved securely after a short pause."</p><section class="form-section"><h3>"Filing period and taxpayer"</h3><div class="form-fields">{COMMON_FIELDS.iter().copied().map(|field|view!{<GuidedInput field=field payload=payload generation=generation revision=revision saving=saving submitting=submitting id=id set_message=set_message/>}).collect_view()}<label class="guided-field"><span>"Quarter"</span><select disabled=move||submitting.get() on:change=set_quarter prop:value=move||payload.get().pointer("/return/period/quarter").and_then(Value::as_i64).unwrap_or_default().to_string()><option value="0">"Select a quarter"</option><option value="1">"First quarter"</option><option value="2">"Second quarter"</option><option value="3">"Third quarter"</option></select><small>"BIR quarterly returns use the first three quarters"</small></label></div></section><section class="form-section"><h3>"Registered details and quarterly figures"</h3><div class="form-fields">{fields.iter().copied().map(|field|view!{<GuidedInput field=field payload=payload generation=generation revision=revision saving=saving submitting=submitting id=id set_message=set_message/>}).collect_view()}</div></section><div class="actions"><span class="save-state">{move||if saving.get(){"Saving…"}else{"All changes saved"}}</span><button class="secondary" disabled=move||submitting.get() on:click=save>"Save now"</button><button disabled=move||submitting.get() on:click=submit>"Send for review"</button></div></>}.into_view()}}</div>}
}

#[component]
fn Operator(
    me: Session,
    set_session: WriteSignal<Option<Session>>,
    message: ReadSignal<String>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let (items, set_items) = create_signal(Vec::<Intake>::new());
    let (selected, set_selected) = create_signal(None::<Intake>);
    let refresh = move || {
        spawn_local(async move {
            match request::<Vec<Intake>>("/operator/intakes", "GET", Value::Null).await {
                Ok(v) => set_items.set(v),
                Err(e) => set_message.set(e),
            }
        })
    };
    refresh();
    view! {<div class="toolbar"><div><p class="eyebrow">"Operator workspace"</p><h1>{format!("Filing inbox · {}",me.email)}</h1></div><button class="secondary" on:click=move |_|logout(set_session,set_message)>"Sign out"</button></div><div class="grid"><section><div class="card"><h2>"Received intakes"</h2><ul class="items">{move||items.get().into_iter().map(|i|{let copy=i.clone();view!{<li><button class="item" on:click=move |_|set_selected.set(Some(copy.clone()))><strong>{format!("{} · {}",i.form_code,i.owner_email)}</strong><span>{i.workflow_status.unwrap_or_default()}</span></button></li>}}).collect_view()}</ul></div><AccountCreator set_message=set_message/></section><section class="card editor">{move||selected.get().map(|i|view!{<OperatorDetail intake=i set_selected=set_selected set_items=set_items set_message=set_message/>}).unwrap_or_else(||view!{<div class="empty"><h2>"Select an intake"</h2></div>}.into_view())}</section></div><p class="error">{move||message.get()}</p>}
}

#[component]
fn AccountCreator(set_message: WriteSignal<String>) -> impl IntoView {
    let (email, set_email) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let (role, set_role) = create_signal("customer".to_string());
    let create = move |_| {
        spawn_local(async move {
            match request::<Value>(
                "/operator/users",
                "POST",
                json!({"email":email.get(),"password":password.get(),"role":role.get()}),
            )
            .await
            {
                Ok(_) => {
                    set_email.set(String::new());
                    set_password.set(String::new());
                    set_message.set("Account created.".into())
                }
                Err(e) => set_message.set(e),
            }
        })
    };
    view! {<section class="card accounts"><h2>"Create account"</h2><p>"There is no public signup. Share credentials with the customer securely."</p><input type="email" placeholder="Email" prop:value=email on:input=move|e|set_email.set(event_target_value(&e))/><input type="password" placeholder="Temporary password (12+ characters)" prop:value=password on:input=move|e|set_password.set(event_target_value(&e))/><select on:change=move|e|set_role.set(event_target_value(&e))><option value="customer">"Customer"</option><option value="operator">"Operator"</option></select><button on:click=create>"Create account"</button></section>}
}

#[component]
fn OperatorDetail(
    intake: Intake,
    set_selected: WriteSignal<Option<Intake>>,
    set_items: WriteSignal<Vec<Intake>>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let id = intake.id;
    let change = move |status: &'static str| {
        spawn_local(async move {
            match request::<Value>(
                &format!("/operator/intakes/{id}/status"),
                "PATCH",
                json!({"status":status}),
            )
            .await
            {
                Ok(_) => set_message.set(format!("Marked {status}.")),
                Err(e) => set_message.set(e),
            }
        })
    };
    let delete = move |_| {
        if !web_sys::window()
            .and_then(|window| {
                window
                    .confirm_with_message(
                        "Permanently delete this intake? The taxpayer data cannot be recovered.",
                    )
                    .ok()
            })
            .unwrap_or(false)
        {
            return;
        }
        spawn_local(async move {
            match request::<Value>(
                &format!("/operator/intakes/{id}"),
                "DELETE",
                json!({"confirm":true}),
            )
            .await
            {
                Ok(_) => {
                    set_selected.set(None);
                    if let Ok(v) =
                        request::<Vec<Intake>>("/operator/intakes", "GET", Value::Null).await
                    {
                        set_items.set(v)
                    }
                }
                Err(e) => set_message.set(e),
            }
        })
    };
    view! {<article class="review"><p class="eyebrow">{intake.workflow_status.unwrap_or_default()}</p><h2>{intake.reference.unwrap_or_default()}</h2><p>{format!("{} · {}",intake.form_code,intake.owner_email)}</p><div class="actions no-print"><a class="button secondary" href=format!("/api/operator/intakes/{id}/export")>"Export JSON"</a><button class="secondary" on:click=move |_|{let _=web_sys::window().unwrap().print();}>"Print review"</button><button on:click=move |_|change("Filed")>"Mark filed"</button><button on:click=move |_|change("Receipt sent")>"Mark receipt sent"</button><button class="danger" on:click=delete>"Delete"</button></div><pre>{serde_json::to_string_pretty(&intake.payload).unwrap()}</pre></article>}
}
