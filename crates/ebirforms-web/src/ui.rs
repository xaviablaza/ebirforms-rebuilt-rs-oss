use crate::*;
use ebirforms_web_schema::{fields_for, set_value, value_at, GuidedField, COMMON_FIELDS};
use leptos::task::spawn_local;
use serde_json::Value;
use std::sync::Arc;

#[allow(deprecated)]
fn error_text(error: leptos::server_fn::ServerFnError<PortalError>) -> String {
    match error {
        leptos::server_fn::ServerFnError::WrappedServerError(error) => error.message().into(),
        other => other.to_string(),
    }
}

pub fn mount_app() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (session, set_session) = signal(None::<Session>);
    let (message, set_message) = signal(String::new());
    let initial_session = Resource::new(|| (), |_| get_session());
    view! {
      <header><a class="brand" href="/">"eBIRForms assisted filing"</a><span>"Secure intake · unofficial"</span></header>
      <main>
        <ErrorBoundary fallback=|_| view! { <section class="card login"><p class="error">"The portal could not be rendered. Please reload and try again."</p></section> }>
        <Suspense fallback=move || view! { <section class="card login"><p>"Loading secure portal…"</p></section> }>
        {move || match session.get().or_else(|| initial_session.get().and_then(Result::ok)) {
          None => view! { <LoginView set_session=set_session message=message set_message=set_message/> }.into_any(),
          Some(me) if me.role == "operator" => view! { <Operator me=me set_session=set_session message=message set_message=set_message/> }.into_any(),
          Some(me) => view! { <Customer me=me set_session=set_session message=message set_message=set_message/> }.into_any(),
        }}
        </Suspense>
        </ErrorBoundary>
      </main>
    }
}

#[component]
fn LoginView(
    set_session: WriteSignal<Option<Session>>,
    message: ReadSignal<String>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let login_action = ServerAction::<Login>::new();
    Effect::new(move || {
        if let Some(result) = login_action.value().get() {
            match result {
                Ok(me) => {
                    set_message.set(String::new());
                    set_session.set(Some(me));
                }
                Err(error) => set_message.set(error_text(error)),
            }
        }
    });
    let submit = move |_| {
        login_action.dispatch(Login {
            email: email.get(),
            password: password.get(),
        });
    };
    view! { <section class="card login"><p class="eyebrow">"Private customer portal"</p><h1>"Continue your assisted filing"</h1><p>"Your account is created by our team before your guided call."</p><label>"Email"<input type="email" on:input=move|e|set_email.set(event_target_value(&e))/></label><label>"Password"<input type="password" on:input=move|e|set_password.set(event_target_value(&e))/></label><button on:click=submit>"Sign in"</button><p class="error">{move||message.get()}</p></section> }
}

fn sign_out(
    me: Session,
    logout_action: ServerAction<Logout>,
    set_session: WriteSignal<Option<Session>>,
    set_message: WriteSignal<String>,
) {
    spawn_local(async move {
        logout_action.dispatch(Logout {
            csrf_token: me.csrf_token,
        });
        while logout_action.pending().get_untracked() {
            gloo_timers::future::TimeoutFuture::new(15).await;
        }
        set_session.set(None);
        set_message.set(String::new());
        if let Some(window) = web_sys::window() {
            let _ = window.location().reload();
        }
    });
}

#[component]
fn Customer(
    me: Session,
    set_session: WriteSignal<Option<Session>>,
    message: ReadSignal<String>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let (items, set_items) = signal(Vec::<Intake>::new());
    let (selected, set_selected) = signal(None::<Intake>);
    let (form_code, set_form_code) = signal("1701Q".to_string());
    let intakes = Resource::new(|| (), |_| list_intakes());
    let create_action = ServerAction::<CreateIntake>::new();
    let logout_action = ServerAction::<Logout>::new();
    Effect::new(move || {
        create_action.version().get();
        intakes.refetch();
    });
    let refresh = move || {
        intakes.refetch();
        spawn_local(async move {
            match intakes.await {
                Ok(v) => set_items.set(v),
                Err(e) => set_message.set(error_text(e)),
            }
        })
    };
    refresh();
    let create_csrf = me.csrf_token.clone();
    let create = move |_| {
        let code = form_code.get();
        let csrf = create_csrf.clone();
        spawn_local(async move {
            create_action.dispatch(CreateIntake {
                form_code: code,
                csrf_token: csrf,
            });
            while create_action.pending().get_untracked() {
                gloo_timers::future::TimeoutFuture::new(15).await;
            }
            match create_action
                .value()
                .get_untracked()
                .expect("create action completed")
            {
                Ok(_) => refresh(),
                Err(e) => set_message.set(error_text(e)),
            }
        })
    };
    let logout_me = me.clone();
    let csrf = me.csrf_token.clone();
    view! { <div class="toolbar"><div><p class="eyebrow">"Customer intake"</p><h1>{format!("Welcome, {}",me.email)}</h1></div><button class="secondary" on:click=move |_|sign_out(logout_me.clone(),logout_action,set_session,set_message)>"Sign out"</button></div>
    <p class="notice">"This portal collects your information for review. It does not file directly with BIR. Our team will file through official eBIRForms and send the official receipt afterward."</p>
    <div class="grid"><section class="card"><h2>"Your returns"</h2><div class="new"><select on:change=move|e|set_form_code.set(event_target_value(&e))><option>"1701Q"</option><option>"1702Q"</option></select><button on:click=create>"Start intake"</button></div><ul class="items">{move||items.get().into_iter().map(|i|{let copy=i.clone();view!{<li><button class="item" on:click=move |_|set_selected.set(Some(copy.clone()))><strong>{i.form_code}</strong><span>{i.reference.unwrap_or_else(||"Draft".into())}</span></button></li>}}).collect_view()}</ul></section>
    <section class="card editor">{move||selected.get().map(|i|view!{<IntakeEditor intake=i csrf_token=csrf.clone() set_selected=set_selected set_items=set_items set_message=set_message/>}.into_any()).unwrap_or_else(||view!{<div class="empty"><h2>"Choose or start an intake"</h2><p>"During your guided call, complete the return data and save as you go."</p></div>}.into_any())}</section></div><p class="error">{move||message.get()}</p> }
}

async fn save_until_clean(
    id: i64,
    payload: RwSignal<Value>,
    revision: RwSignal<i64>,
    generation: RwSignal<u64>,
    saving: RwSignal<bool>,
    csrf_token: String,
    save_action: ServerAction<SaveIntake>,
) -> Result<(), String> {
    while saving.get_untracked() {
        gloo_timers::future::TimeoutFuture::new(30).await;
    }
    saving.set(true);
    loop {
        let current_generation = generation.get_untracked();
        save_action.dispatch(SaveIntake {
            id,
            payload: payload.get_untracked(),
            revision: revision.get_untracked(),
            csrf_token: csrf_token.clone(),
        });
        while save_action.pending().get_untracked() {
            gloo_timers::future::TimeoutFuture::new(15).await;
        }
        match save_action
            .value()
            .get_untracked()
            .expect("save action completed")
        {
            Ok(value) => revision.set(value.revision),
            Err(error) => {
                saving.set(false);
                return Err(error_text(error));
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
    csrf_token: String,
    save_action: ServerAction<SaveIntake>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let update = Arc::new(move |value: String| {
        payload.update(|data| {
            set_value(data, field.path, &value);
            if field.input_type == "atc_1701" {
                let eight_percent = matches!(value.as_str(), "II015" | "II017" | "II016");
                set_value(
                    data,
                    "guided.tax_regime",
                    if eight_percent {
                        "eight_percent"
                    } else {
                        "graduated"
                    },
                );
                if eight_percent {
                    set_value(data, "guided.deduction_method", "");
                    set_value(data, "guided.cost", "0");
                    set_value(data, "guided.itemized_deductions", "0");
                }
            } else if field.path == "guided.deduction_method" && value == "osd" {
                set_value(data, "guided.cost", "0");
                set_value(data, "guided.itemized_deductions", "0");
            }
        });
        generation.update(|value| *value += 1);
        let expected = generation.get_untracked();
        let csrf = csrf_token.clone();
        spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(650).await;
            if generation.get_untracked() == expected {
                if let Err(error) =
                    save_until_clean(id, payload, revision, generation, saving, csrf, save_action)
                        .await
                {
                    set_message.set(error)
                }
            }
        })
    });
    let disabled = move || {
        if submitting.get() {
            return true;
        }
        let data = payload.get();
        let eight_percent = matches!(
            value_at(&data, "guided.atc").as_str(),
            "II015" | "II017" | "II016"
        );
        field.input_type == "tax_regime"
            || (field.path == "guided.deduction_method" && eight_percent)
            || (matches!(field.path, "guided.cost" | "guided.itemized_deductions")
                && (eight_percent || value_at(&data, "guided.deduction_method") == "osd"))
    };
    let choices: &[(&str, &str)] = match field.input_type {
        "quarter" => &[
            ("", "Select a quarter"),
            ("1", "First quarter"),
            ("2", "Second quarter"),
            ("3", "Third quarter"),
        ],
        "yes_no" => &[("", "Select an answer"), ("no", "No"), ("yes", "Yes")],
        "taxpayer_type" => &[
            ("", "Select taxpayer type"),
            ("single", "Single proprietor"),
            ("professional", "Professional"),
            ("estate", "Estate"),
            ("trust", "Trust"),
        ],
        "atc_1701" => &[
            ("", "Select ATC"),
            ("II012", "II012"),
            ("II014", "II014"),
            ("II013", "II013"),
            ("II015", "II015"),
            ("II017", "II017"),
            ("II016", "II016"),
        ],
        "atc_1702" => &[
            ("", "Select ATC"),
            ("WC160", "WC160"),
            ("WC170", "WC170"),
            ("WC180", "WC180"),
        ],
        "tax_regime" => &[
            ("", "Select tax regime"),
            ("graduated", "Graduated income tax rates"),
            ("eight_percent", "8% income tax rate"),
        ],
        "deduction_method" => &[
            ("", "Select deduction method"),
            ("itemized", "Itemized deductions"),
            ("osd", "Optional standard deduction"),
        ],
        "entity_type" => &[
            ("", "Select entity type"),
            ("domestic", "Domestic corporation"),
            ("resident_foreign", "Resident foreign corporation"),
            ("nonresident_foreign", "Non-resident foreign corporation"),
        ],
        _ => &[],
    };
    let control = if choices.is_empty() {
        let update = Arc::clone(&update);
        view!{<input type=field.input_type disabled=disabled prop:value=move||value_at(&payload.get(),field.path) on:input=move|event|update(event_target_value(&event))/>}.into_any()
    } else {
        let update = Arc::clone(&update);
        view!{<select disabled=disabled prop:value=move||value_at(&payload.get(),field.path) on:change=move|event|update(event_target_value(&event))>{choices.iter().map(|(value,label)|view!{<option value=*value>{*label}</option>}).collect_view()}</select>}.into_any()
    };
    view! {<label class="guided-field"><span>{field.label}</span>{control}<small>{field.hint}</small></label>}
}

#[component]
fn IntakeEditor(
    intake: Intake,
    csrf_token: String,
    set_selected: WriteSignal<Option<Intake>>,
    set_items: WriteSignal<Vec<Intake>>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let payload = RwSignal::new(intake.payload.clone());
    let revision = RwSignal::new(intake.revision);
    let generation = RwSignal::new(0_u64);
    let saving = RwSignal::new(false);
    let submitting = RwSignal::new(false);
    let save_action = ServerAction::<SaveIntake>::new();
    let submit_action = ServerAction::<SubmitIntake>::new();
    let id = intake.id;
    let detail = Resource::new(move || id, get_intake);
    let locked = intake.state != "draft";
    let fields = fields_for(&intake.form_code);
    let save_csrf = csrf_token.clone();
    let save = move |_| {
        let csrf = save_csrf.clone();
        spawn_local(async move {
            match save_until_clean(id, payload, revision, generation, saving, csrf, save_action)
                .await
            {
                Ok(()) => set_message.set("Draft saved.".into()),
                Err(error) => set_message.set(error),
            }
        })
    };
    let submit_csrf = csrf_token.clone();
    let submit = move |_| {
        let csrf = submit_csrf.clone();
        spawn_local(async move {
            submitting.set(true);
            if let Err(error) = save_until_clean(
                id,
                payload,
                revision,
                generation,
                saving,
                csrf.clone(),
                save_action,
            )
            .await
            {
                submitting.set(false);
                set_message.set(error);
                return;
            }
            submit_action.dispatch(SubmitIntake {
                id,
                csrf_token: csrf,
            });
            while submit_action.pending().get_untracked() {
                gloo_timers::future::TimeoutFuture::new(15).await;
            }
            match submit_action
                .value()
                .get_untracked()
                .expect("submit action completed")
            {
                Ok(value) => {
                    set_message.set(value.message);
                    set_selected.set(None);
                    if let Ok(items) = list_intakes().await {
                        set_items.set(items)
                    }
                }
                Err(error) => {
                    submitting.set(false);
                    set_message.set(error_text(error))
                }
            }
        })
    };
    let common_csrf = csrf_token.clone();
    let fields_csrf = csrf_token;
    view! {<><Transition fallback=move || view!{<p>"Loading intake…"</p>}>{move || detail.get().map(|_| ())}</Transition><div><p class="eyebrow">{format!("{} guided intake",intake.form_code)}</p><h2>{intake.reference.clone().unwrap_or_else(||"Draft return".into())}</h2>{if locked{view!{<div class="success"><h3>"Information received"</h3><p>"Our team will review and file this return. The official receipt will follow after filing."</p></div>}.into_any()}else{view!{<><p>"Complete each section with your filing adviser. Changes are saved securely after a short pause."</p><section class="form-section"><h3>"Filing period and taxpayer"</h3><div class="form-fields">{COMMON_FIELDS.iter().copied().map(|field|view!{<GuidedInput field=field payload=payload generation=generation revision=revision saving=saving submitting=submitting id=id csrf_token=common_csrf.clone() save_action=save_action set_message=set_message/>}).collect_view()}</div></section><section class="form-section"><h3>"Registered details, filing choices, and quarterly figures"</h3><div class="form-fields">{fields.iter().copied().map(|field|view!{<GuidedInput field=field payload=payload generation=generation revision=revision saving=saving submitting=submitting id=id csrf_token=fields_csrf.clone() save_action=save_action set_message=set_message/>}).collect_view()}</div></section><div class="actions"><span class="save-state">{move||if saving.get(){"Saving…"}else{"All changes saved"}}</span><button class="secondary" disabled=move||submitting.get() on:click=save>"Save now"</button><button disabled=move||submitting.get() on:click=submit>"Send for review"</button></div></>}.into_any()}}</div></>}
}

#[component]
fn Operator(
    me: Session,
    set_session: WriteSignal<Option<Session>>,
    message: ReadSignal<String>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let (items, set_items) = signal(Vec::<Intake>::new());
    let (selected, set_selected) = signal(None::<Intake>);
    let operator_intakes = Resource::new(|| (), |_| operator_list_intakes());
    let logout_action = ServerAction::<Logout>::new();
    let refresh = move || {
        operator_intakes.refetch();
        spawn_local(async move {
            match operator_intakes.await {
                Ok(v) => set_items.set(v),
                Err(e) => set_message.set(error_text(e)),
            }
        })
    };
    refresh();
    let logout_me = me.clone();
    let account_csrf = me.csrf_token.clone();
    let detail_csrf = me.csrf_token;
    view! {<div class="toolbar"><div><p class="eyebrow">"Operator workspace"</p><h1>{format!("Filing inbox · {}",me.email)}</h1></div><button class="secondary" on:click=move |_|sign_out(logout_me.clone(),logout_action,set_session,set_message)>"Sign out"</button></div><div class="grid"><section><div class="card"><h2>"Received intakes"</h2><ul class="items">{move||items.get().into_iter().map(|i|{let copy=i.clone();view!{<li><button class="item" on:click=move |_|set_selected.set(Some(copy.clone()))><strong>{format!("{} · {}",i.form_code,i.owner_email)}</strong><span>{i.workflow_status.unwrap_or_default()}</span></button></li>}}).collect_view()}</ul></div><AccountCreator csrf_token=account_csrf set_message=set_message/></section><section class="card editor">{move||selected.get().map(|i|view!{<OperatorDetail intake=i csrf_token=detail_csrf.clone() set_selected=set_selected set_items=set_items set_message=set_message/>}.into_any()).unwrap_or_else(||view!{<div class="empty"><h2>"Select an intake"</h2></div>}.into_any())}</section></div><p class="error">{move||message.get()}</p>}
}

#[component]
fn AccountCreator(csrf_token: String, set_message: WriteSignal<String>) -> impl IntoView {
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (role, set_role) = signal("customer".to_string());
    let account_action = ServerAction::<OperatorCreateAccount>::new();
    let create = move |_| {
        let csrf = csrf_token.clone();
        spawn_local(async move {
            account_action.dispatch(OperatorCreateAccount {
                email: email.get(),
                password: password.get(),
                role: role.get(),
                csrf_token: csrf,
            });
            while account_action.pending().get_untracked() {
                gloo_timers::future::TimeoutFuture::new(15).await;
            }
            match account_action
                .value()
                .get_untracked()
                .expect("account action completed")
            {
                Ok(_) => {
                    set_email.set(String::new());
                    set_password.set(String::new());
                    set_message.set("Account created.".into())
                }
                Err(e) => set_message.set(error_text(e)),
            }
        })
    };
    view! {<section class="card accounts"><h2>"Create account"</h2><p>"There is no public signup. Share credentials with the customer securely."</p><input type="email" placeholder="Email" prop:value=email on:input=move|e|set_email.set(event_target_value(&e))/><input type="password" placeholder="Temporary password (12+ characters)" prop:value=password on:input=move|e|set_password.set(event_target_value(&e))/><select on:change=move|e|set_role.set(event_target_value(&e))><option value="customer">"Customer"</option><option value="operator">"Operator"</option></select><button on:click=create>"Create account"</button></section>}
}

#[component]
fn OperatorDetail(
    intake: Intake,
    csrf_token: String,
    set_selected: WriteSignal<Option<Intake>>,
    set_items: WriteSignal<Vec<Intake>>,
    set_message: WriteSignal<String>,
) -> impl IntoView {
    let id = intake.id;
    let status_action = ServerAction::<OperatorUpdateStatus>::new();
    let delete_action = ServerAction::<OperatorDeleteIntake>::new();
    let detail = Resource::new(move || id, operator_get_intake);
    let status_csrf = csrf_token.clone();
    let change = Arc::new(move |status: &'static str| {
        let csrf = status_csrf.clone();
        spawn_local(async move {
            status_action.dispatch(OperatorUpdateStatus {
                id,
                status: status.into(),
                csrf_token: csrf,
            });
            while status_action.pending().get_untracked() {
                gloo_timers::future::TimeoutFuture::new(15).await;
            }
            match status_action
                .value()
                .get_untracked()
                .expect("status action completed")
            {
                Ok(_) => {
                    set_items.update(|items| {
                        if let Some(item) = items.iter_mut().find(|item| item.id == id) {
                            item.workflow_status = Some(status.into())
                        }
                    });
                    set_selected.update(|item| {
                        if let Some(item) = item.as_mut() {
                            if item.id == id {
                                item.workflow_status = Some(status.into())
                            }
                        }
                    });
                    set_message.set(format!("Marked {status}."));
                }
                Err(e) => set_message.set(error_text(e)),
            }
        })
    });
    let delete_csrf = csrf_token;
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
        let csrf = delete_csrf.clone();
        spawn_local(async move {
            delete_action.dispatch(OperatorDeleteIntake {
                id,
                confirm: true,
                csrf_token: csrf,
            });
            while delete_action.pending().get_untracked() {
                gloo_timers::future::TimeoutFuture::new(15).await;
            }
            match delete_action
                .value()
                .get_untracked()
                .expect("delete action completed")
            {
                Ok(_) => {
                    set_selected.set(None);
                    if let Ok(v) = operator_list_intakes().await {
                        set_items.set(v)
                    }
                }
                Err(e) => set_message.set(error_text(e)),
            }
        })
    };
    let mark_filed = Arc::clone(&change);
    let mark_receipt = change;
    view! {<><Transition fallback=move || view!{<p>"Loading intake…"</p>}>{move || detail.get().map(|_| ())}</Transition><article class="review"><p class="eyebrow">{intake.workflow_status.unwrap_or_default()}</p><h2>{intake.reference.unwrap_or_default()}</h2><p>{format!("{} · {}",intake.form_code,intake.owner_email)}</p><div class="actions no-print"><a class="button secondary" href=format!("/api/operator/intakes/{id}/export")>"Export JSON"</a><button class="secondary" on:click=move |_|{let _=web_sys::window().unwrap().print();}>"Print review"</button><button on:click=move |_|mark_filed("Filed")>"Mark filed"</button><button on:click=move |_|mark_receipt("Receipt sent")>"Mark receipt sent"</button><button class="danger" on:click=delete>"Delete"</button></div><pre>{serde_json::to_string_pretty(&intake.payload).unwrap()}</pre></article></>}
}
