#![allow(clippy::type_complexity)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use secrecy::SecretBox;
use tokio::runtime::Handle;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::TeamMemberRole;
use crate::services::team_service::TeamService;
use crate::services::vault_service::VaultService;

pub struct ManageTeamsDialog {
    window: gtk4::Window,
}

impl ManageTeamsDialog {
    #[allow(clippy::too_many_arguments)]
    pub fn new<TTeam, TVault>(
        application: &adw::Application,
        parent: &adw::ApplicationWindow,
        runtime_handle: Handle,
        team_service: Arc<TTeam>,
        vault_service: Arc<TVault>,
        actor_user_id: Uuid,
        session_master_key: Rc<RefCell<Vec<u8>>>,
        _active_vault_id: Rc<RefCell<Option<Uuid>>>,
        on_state_changed: Rc<dyn Fn()>,
    ) -> Self
    where
        TTeam: TeamService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        let window = gtk4::Window::builder()
            .application(application)
            .transient_for(parent)
            .title(crate::tr!("manage-teams-window-title").as_str())
            .modal(true)
            .default_width(820)
            .default_height(620)
            .build();

        {
            let on_state_changed_for_hide = Rc::clone(&on_state_changed);
            window.connect_hide(move |_| {
                on_state_changed_for_hide();
            });
        }

        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let header = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();
        let title = gtk4::Label::new(Some(crate::tr!("manage-teams-title").as_str()));
        title.add_css_class("title-3");
        title.set_hexpand(true);
        title.set_halign(gtk4::Align::Start);
        let create_btn =
            gtk4::Button::with_label(crate::tr!("manage-teams-create-button").as_str());
        create_btn.add_css_class("suggested-action");
        header.append(&title);
        header.append(&create_btn);

        let scrolled = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();
        let team_list = gtk4::ListBox::new();
        team_list.add_css_class("boxed-list");
        team_list.set_selection_mode(gtk4::SelectionMode::None);
        scrolled.set_child(Some(&team_list));

        root.append(&header);
        root.append(&scrolled);
        window.set_child(Some(&root));

        let refresh_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        let refresh: Rc<dyn Fn()> = {
            let team_list_for_refresh = team_list.clone();
            let runtime_for_refresh = runtime_handle.clone();
            let team_for_refresh = Arc::clone(&team_service);
            let vault_for_refresh = Arc::clone(&vault_service);
            let window_for_refresh = window.clone();
            let master_for_refresh = Rc::clone(&session_master_key);
            let refresh_holder_for_rows = Rc::clone(&refresh_holder);
            Rc::new(move || {
                while let Some(child) = team_list_for_refresh.first_child() {
                    team_list_for_refresh.remove(&child);
                }

                let (sender, receiver) = tokio::sync::oneshot::channel();
                let runtime_for_task = runtime_for_refresh.clone();
                let team_for_task = Arc::clone(&team_for_refresh);
                std::thread::spawn(move || {
                    let result = runtime_for_task.block_on(async move {
                        team_for_task.list_visible_teams(actor_user_id).await
                    });
                    let _ = sender.send(result);
                });

                let team_list_for_recv = team_list_for_refresh.clone();
                let runtime_for_recv = runtime_for_refresh.clone();
                let team_for_recv = Arc::clone(&team_for_refresh);
                let vault_for_recv = Arc::clone(&vault_for_refresh);
                let window_for_recv = window_for_refresh.clone();
                let master_for_recv = Rc::clone(&master_for_refresh);
                let refresh_holder_for_rows_async = Rc::clone(&refresh_holder_for_rows);
                let on_state_changed_for_rows = Rc::clone(&on_state_changed);
                glib::MainContext::default().spawn_local(async move {
                    match receiver.await {
                        Ok(Ok(teams)) => {
                            for team in teams {
                                let row_box = gtk4::Box::builder()
                                    .orientation(gtk4::Orientation::Horizontal)
                                    .spacing(8)
                                    .margin_top(8)
                                    .margin_bottom(8)
                                    .margin_start(8)
                                    .margin_end(8)
                                    .build();

                                let name = gtk4::Label::new(Some(team.name.as_str()));
                                name.set_halign(gtk4::Align::Start);
                                name.set_hexpand(true);

                                let created = gtk4::Label::new(Some(team.created_at.as_str()));
                                created.set_halign(gtk4::Align::Start);
                                created.set_width_chars(22);

                                let members_btn = gtk4::Button::with_label(crate::tr!("manage-teams-members-button").as_str());
                                members_btn.add_css_class("flat");
                                let share_btn = gtk4::Button::with_label(crate::tr!("manage-teams-share-vault-button").as_str());
                                share_btn.add_css_class("flat");
                                let delete_btn = gtk4::Button::with_label(crate::tr!("manage-teams-delete-button").as_str());
                                delete_btn.add_css_class("destructive-action");

                                row_box.append(&name);
                                row_box.append(&created);
                                row_box.append(&members_btn);
                                row_box.append(&share_btn);
                                row_box.append(&delete_btn);

                                let row = gtk4::ListBoxRow::new();
                                row.set_child(Some(&row_box));
                                team_list_for_recv.append(&row);

                                let team_id = team.id;
                                let team_name = team.name.clone();
                                let runtime_for_members = runtime_for_recv.clone();
                                let team_for_members = Arc::clone(&team_for_recv);
                                let window_for_members = window_for_recv.clone();
                                members_btn.connect_clicked(move |_| {
                                    let members_window = gtk4::Window::builder()
                                        .title(crate::tr!("manage-teams-members-window-title").as_str())
                                        .modal(true)
                                        .transient_for(&window_for_members)
                                        .default_width(620)
                                        .default_height(500)
                                        .build();

                                    let members_root = gtk4::Box::builder()
                                        .orientation(gtk4::Orientation::Vertical)
                                        .spacing(8)
                                        .margin_top(12)
                                        .margin_bottom(12)
                                        .margin_start(12)
                                        .margin_end(12)
                                        .build();

                                    let members_list = gtk4::ListBox::new();
                                    members_list.add_css_class("boxed-list");
                                    members_list.set_selection_mode(gtk4::SelectionMode::None);
                                    let members_scroll = gtk4::ScrolledWindow::builder()
                                        .vexpand(true)
                                        .hexpand(true)
                                        .build();
                                    members_scroll.set_child(Some(&members_list));

                                    let controls = gtk4::Box::builder()
                                        .orientation(gtk4::Orientation::Horizontal)
                                        .spacing(8)
                                        .build();
                                    let user_picker = gtk4::DropDown::from_strings(&[crate::tr!("manage-teams-member-picker-placeholder").as_str()]);
                                    user_picker.set_hexpand(true);
                                    let role_picker = gtk4::DropDown::from_strings(&[
                                        crate::tr!("manage-teams-role-member").as_str(),
                                        crate::tr!("manage-teams-role-leader").as_str(),
                                    ]);
                                    role_picker.set_selected(0);
                                    let add_btn = gtk4::Button::with_label(crate::tr!("manage-teams-add-member-button").as_str());
                                    add_btn.add_css_class("suggested-action");
                                    controls.append(&user_picker);
                                    controls.append(&role_picker);
                                    controls.append(&add_btn);

                                    members_root.append(&members_scroll);
                                    members_root.append(&controls);
                                    members_window.set_child(Some(&members_root));

                                    let picker_ids: Rc<RefCell<Vec<Uuid>>> = Rc::new(RefCell::new(Vec::new()));
                                    let refresh_members_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> =
                                        Rc::new(RefCell::new(None));
                                    let refresh_members: Rc<dyn Fn()> = {
                                        let members_list_for_refresh = members_list.clone();
                                        let runtime_for_refresh_inner = runtime_for_members.clone();
                                        let team_for_refresh_inner = Arc::clone(&team_for_members);
                                        let window_for_refresh_inner = members_window.clone();
                                        let picker_for_refresh = user_picker.clone();
                                        let picker_ids_for_refresh = Rc::clone(&picker_ids);
                                        let refresh_members_holder_for_rows = Rc::clone(&refresh_members_holder);
                                        Rc::new(move || {
                                            while let Some(child) = members_list_for_refresh.first_child() {
                                                members_list_for_refresh.remove(&child);
                                            }

                                            let (sender, receiver) = tokio::sync::oneshot::channel();
                                            let runtime_for_task_inner = runtime_for_refresh_inner.clone();
                                            let team_for_task_inner = Arc::clone(&team_for_refresh_inner);
                                            std::thread::spawn(move || {
                                                let result = runtime_for_task_inner.block_on(async move {
                                                    let members = team_for_task_inner.list_team_members(actor_user_id, team_id).await?;
                                                    let users = team_for_task_inner.list_users_for_member_picker(actor_user_id).await?;
                                                    Ok::<_, AppError>((members, users))
                                                });
                                                let _ = sender.send(result);
                                            });

                                            let members_list_for_recv_inner = members_list_for_refresh.clone();
                                            let window_for_recv_inner = window_for_refresh_inner.clone();
                                            let team_for_recv_inner = Arc::clone(&team_for_refresh_inner);
                                            let runtime_for_recv_inner = runtime_for_refresh_inner.clone();
                                            let picker_for_recv = picker_for_refresh.clone();
                                            let picker_ids_for_recv = Rc::clone(&picker_ids_for_refresh);
                                            let refresh_members_holder_for_rows_async =
                                                Rc::clone(&refresh_members_holder_for_rows);
                                            glib::MainContext::default().spawn_local(async move {
                                                match receiver.await {
                                                    Ok(Ok((members, users))) => {
                                                        let ids: Vec<Uuid> = users.iter().map(|u| u.id).collect();
                                                        *picker_ids_for_recv.borrow_mut() = ids;
                                                        let labels: Vec<String> = std::iter::once(crate::tr!("manage-teams-member-picker-placeholder").to_string())
                                                            .chain(users.iter().map(|u| u.username.clone()))
                                                            .collect();
                                                        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                                                        picker_for_recv.set_model(Some(&gtk4::StringList::new(&refs)));
                                                        picker_for_recv.set_selected(0);

                                                        for member in members {
                                                            let row_box = gtk4::Box::builder()
                                                                .orientation(gtk4::Orientation::Horizontal)
                                                                .spacing(8)
                                                                .margin_top(6)
                                                                .margin_bottom(6)
                                                                .margin_start(6)
                                                                .margin_end(6)
                                                                .build();
                                                            let member_label = gtk4::Label::new(Some(member.user_id.to_string().as_str()));
                                                            member_label.set_hexpand(true);
                                                            member_label.set_halign(gtk4::Align::Start);
                                                            let role_text = match member.role {
                                                                TeamMemberRole::Leader => crate::tr!("manage-teams-role-leader").to_string(),
                                                                TeamMemberRole::Member => crate::tr!("manage-teams-role-member").to_string(),
                                                            };
                                                            let role_label = gtk4::Label::new(Some(role_text.as_str()));
                                                            role_label.set_width_chars(10);
                                                            let remove_btn = gtk4::Button::with_label(crate::tr!("manage-teams-remove-member-button").as_str());
                                                            remove_btn.add_css_class("destructive-action");
                                                            row_box.append(&member_label);
                                                            row_box.append(&role_label);
                                                            row_box.append(&remove_btn);

                                                            let row = gtk4::ListBoxRow::new();
                                                            row.set_child(Some(&row_box));
                                                            members_list_for_recv_inner.append(&row);

                                                            let remove_user_id = member.user_id;
                                                            let team_for_remove = Arc::clone(&team_for_recv_inner);
                                                            let runtime_for_remove = runtime_for_recv_inner.clone();
                                                            let window_for_remove = window_for_recv_inner.clone();
                                                            let refresh_holder_for_remove =
                                                                Rc::clone(&refresh_members_holder_for_rows_async);
                                                            remove_btn.connect_clicked(move |_| {
                                                                let (sender, receiver) = tokio::sync::oneshot::channel();
                                                                let runtime_for_task = runtime_for_remove.clone();
                                                                let team_for_task = Arc::clone(&team_for_remove);
                                                                std::thread::spawn(move || {
                                                                    let result = runtime_for_task.block_on(async move {
                                                                        team_for_task.remove_member(actor_user_id, team_id, remove_user_id).await
                                                                    });
                                                                    let _ = sender.send(result);
                                                                });
                                                                let window_for_result = window_for_remove.clone();
                                                                let refresh_holder_for_result = Rc::clone(&refresh_holder_for_remove);
                                                                glib::MainContext::default().spawn_local(async move {
                                                                    match receiver.await {
                                                                        Ok(Ok(())) => {
                                                                            if let Some(refresh_cb) = refresh_holder_for_result.borrow().as_ref() {
                                                                                refresh_cb();
                                                                            }
                                                                        }
                                                                        Ok(Err(err)) => Self::show_error(&window_for_result, err),
                                                                        Err(_) => Self::show_error(&window_for_result, AppError::Internal),
                                                                    }
                                                                });
                                                            });
                                                        }
                                                    }
                                                    Ok(Err(err)) => Self::show_error(&window_for_recv_inner, err),
                                                    Err(_) => Self::show_error(&window_for_recv_inner, AppError::Internal),
                                                }
                                            });
                                        })
                                    };

                                    let team_for_add = Arc::clone(&team_for_members);
                                    let runtime_for_add = runtime_for_members.clone();
                                    let window_for_add = members_window.clone();
                                    let picker_ids_for_add = Rc::clone(&picker_ids);
                                    let refresh_members_for_add = Rc::clone(&refresh_members_holder);
                                    add_btn.connect_clicked(move |_| {
                                        let selected = user_picker.selected();
                                        if selected == 0 {
                                            Self::show_error(&window_for_add, AppError::Validation(crate::tr!("manage-teams-error-select-user").to_string()));
                                            return;
                                        }
                                        let index = (selected - 1) as usize;
                                        let ids = picker_ids_for_add.borrow();
                                        if index >= ids.len() {
                                            Self::show_error(&window_for_add, AppError::Validation(crate::tr!("manage-teams-error-invalid-user").to_string()));
                                            return;
                                        }
                                        let user_id = ids[index];
                                        let role = if role_picker.selected() == 1 {
                                            TeamMemberRole::Leader
                                        } else {
                                            TeamMemberRole::Member
                                        };

                                        let (sender, receiver) = tokio::sync::oneshot::channel();
                                        let runtime_for_task = runtime_for_add.clone();
                                        let team_for_task = Arc::clone(&team_for_add);
                                        std::thread::spawn(move || {
                                            let result = runtime_for_task.block_on(async move {
                                                team_for_task.add_member(actor_user_id, team_id, user_id, role).await
                                            });
                                            let _ = sender.send(result);
                                        });
                                        let window_for_result = window_for_add.clone();
                                        let refresh_after_result = Rc::clone(&refresh_members_for_add);
                                        glib::MainContext::default().spawn_local(async move {
                                            match receiver.await {
                                                Ok(Ok(())) => {
                                                    if let Some(refresh_cb) = refresh_after_result.borrow().as_ref() {
                                                        refresh_cb();
                                                    }
                                                }
                                                Ok(Err(err)) => Self::show_error(&window_for_result, err),
                                                Err(_) => Self::show_error(&window_for_result, AppError::Internal),
                                            }
                                        });
                                    });

                                    *refresh_members_holder.borrow_mut() = Some(refresh_members.clone());
                                    refresh_members();
                                    members_window.present();
                                });

                                let runtime_for_share = runtime_for_recv.clone();
                                let team_for_share = Arc::clone(&team_for_recv);
                                let vault_for_share = Arc::clone(&vault_for_recv);
                                let window_for_share = window_for_recv.clone();
                                let master_for_share = Rc::clone(&master_for_recv);
                                let on_state_changed_for_share = Rc::clone(&on_state_changed_for_rows);
                                share_btn.connect_clicked(move |_| {
                                    let current_master = master_for_share.borrow().clone();
                                    if current_master.is_empty() {
                                        Self::show_warning(
                                            &window_for_share,
                                            crate::tr!("manage-teams-warning-vault-locked-title").as_str(),
                                            crate::tr!("manage-teams-warning-vault-locked-body").as_str(),
                                        );
                                        return;
                                    }

                                    let share_window = gtk4::Window::builder()
                                        .title(crate::tr!("manage-teams-share-select-window-title").as_str())
                                        .modal(true)
                                        .transient_for(&window_for_share)
                                        .default_width(520)
                                        .default_height(140)
                                        .build();

                                    let share_root = gtk4::Box::builder()
                                        .orientation(gtk4::Orientation::Vertical)
                                        .spacing(8)
                                        .margin_top(12)
                                        .margin_bottom(12)
                                        .margin_start(12)
                                        .margin_end(12)
                                        .build();

                                    let vault_picker = gtk4::DropDown::from_strings(&[
                                        crate::tr!("manage-teams-share-select-placeholder").as_str(),
                                    ]);
                                    vault_picker.set_hexpand(true);

                                    let status_label = gtk4::Label::new(None);
                                    status_label.set_halign(gtk4::Align::Start);
                                    status_label.add_css_class("dim-label");

                                    let actions = gtk4::Box::builder()
                                        .orientation(gtk4::Orientation::Horizontal)
                                        .spacing(8)
                                        .halign(gtk4::Align::End)
                                        .build();
                                    let cancel_btn = gtk4::Button::with_label(crate::tr!("common-cancel").as_str());
                                    let share_apply_btn = gtk4::Button::with_label(crate::tr!("manage-teams-share-select-apply").as_str());
                                    share_apply_btn.add_css_class("suggested-action");
                                    actions.append(&cancel_btn);
                                    actions.append(&share_apply_btn);

                                    share_root.append(&vault_picker);
                                    share_root.append(&status_label);
                                    share_root.append(&actions);
                                    share_window.set_child(Some(&share_root));

                                    let vault_picker_ids: Rc<RefCell<Vec<Uuid>>> = Rc::new(RefCell::new(Vec::new()));

                                    {
                                        let runtime_for_list = runtime_for_share.clone();
                                        let vault_for_list = Arc::clone(&vault_for_share);
                                        let picker_for_list = vault_picker.clone();
                                        let picker_ids_for_list = Rc::clone(&vault_picker_ids);
                                        let status_for_list = status_label.clone();
                                        let apply_for_list = share_apply_btn.clone();
                                        let team_name_for_list = team_name.clone();

                                        let (sender, receiver) = tokio::sync::oneshot::channel();
                                        std::thread::spawn(move || {
                                            let result = runtime_for_list.block_on(async move {
                                                vault_for_list.list_owned_vaults(actor_user_id).await
                                            });
                                            let _ = sender.send(result);
                                        });

                                        glib::MainContext::default().spawn_local(async move {
                                            match receiver.await {
                                                Ok(Ok(vaults)) => {
                                                    if vaults.is_empty() {
                                                        apply_for_list.set_sensitive(false);
                                                        status_for_list.set_text(
                                                            crate::tr!("manage-teams-share-select-empty").as_str(),
                                                        );
                                                        return;
                                                    }

                                                    let mut ids = Vec::with_capacity(vaults.len());
                                                    let labels: Vec<String> = std::iter::once(
                                                        crate::tr!("manage-teams-share-select-placeholder").to_string(),
                                                    )
                                                    .chain(vaults.iter().map(|vault| {
                                                        ids.push(vault.id);
                                                        vault.name.clone()
                                                    }))
                                                    .collect();

                                                    *picker_ids_for_list.borrow_mut() = ids;
                                                    let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                                                    picker_for_list.set_model(Some(&gtk4::StringList::new(&refs)));
                                                    picker_for_list.set_selected(0);
                                                    status_for_list.set_text(
                                                        crate::i18n::tr_args(
                                                            "manage-teams-share-select-status",
                                                            &[("team", crate::i18n::I18nArg::Str(team_name_for_list.as_str()))],
                                                        )
                                                        .as_str(),
                                                    );
                                                }
                                                _ => {
                                                    apply_for_list.set_sensitive(false);
                                                    status_for_list.set_text(
                                                        crate::tr!("main-list-unavailable-description").as_str(),
                                                    );
                                                }
                                            }
                                        });
                                    }

                                    let share_window_for_cancel = share_window.clone();
                                    cancel_btn.connect_clicked(move |_| share_window_for_cancel.close());

                                    let runtime_for_apply = runtime_for_share.clone();
                                    let team_for_apply = Arc::clone(&team_for_share);
                                    let vault_for_apply = Arc::clone(&vault_for_share);
                                    let picker_ids_for_apply = Rc::clone(&vault_picker_ids);
                                    let picker_for_apply = vault_picker.clone();
                                    let window_for_result = window_for_share.clone();
                                    let share_window_for_apply = share_window.clone();
                                    let on_state_changed_for_result = Rc::clone(&on_state_changed_for_share);
                                    share_apply_btn.connect_clicked(move |_| {
                                        let selected = picker_for_apply.selected();
                                        if selected == 0 {
                                            Self::show_error(
                                                &window_for_result,
                                                AppError::Validation(
                                                    crate::tr!("manage-teams-error-select-vault").to_string(),
                                                ),
                                            );
                                            return;
                                        }

                                        let index = (selected - 1) as usize;
                                        let ids = picker_ids_for_apply.borrow();
                                        if index >= ids.len() {
                                            Self::show_error(
                                                &window_for_result,
                                                AppError::Validation(
                                                    crate::tr!("manage-teams-error-select-vault").to_string(),
                                                ),
                                            );
                                            return;
                                        }

                                        let vault_id = ids[index];
                                        let master_for_task = current_master.clone();

                                        let (sender, receiver) = tokio::sync::oneshot::channel();
                                        let runtime_for_task = runtime_for_apply.clone();
                                        let team_for_task = Arc::clone(&team_for_apply);
                                        let vault_for_task = Arc::clone(&vault_for_apply);
                                        std::thread::spawn(move || {
                                            let result = runtime_for_task.block_on(async move {
                                                let vault_key = vault_for_task
                                                    .open_vault_for_user(
                                                        actor_user_id,
                                                        vault_id,
                                                        SecretBox::new(Box::new(master_for_task)),
                                                    )
                                                    .await?;

                                                team_for_task
                                                    .share_vault_with_team(
                                                        actor_user_id,
                                                        vault_id,
                                                        team_id,
                                                        vault_key,
                                                        &[],
                                                    )
                                                    .await
                                            });
                                            let _ = sender.send(result);
                                        });

                                        let share_window_close = share_window_for_apply.clone();
                                        let window_for_result_inner = window_for_result.clone();
                                        let on_state_changed_for_result_inner = Rc::clone(&on_state_changed_for_result);
                                        glib::MainContext::default().spawn_local(async move {
                                            match receiver.await {
                                                Ok(Ok(())) => {
                                                    share_window_close.close();
                                                    on_state_changed_for_result_inner();
                                                    Self::show_warning(
                                                        &window_for_result_inner,
                                                        crate::tr!("manage-teams-warning-share-title").as_str(),
                                                        crate::tr!("manage-teams-warning-share-body").as_str(),
                                                    );
                                                }
                                                Ok(Err(err)) => Self::show_error(&window_for_result_inner, err),
                                                Err(_) => Self::show_error(&window_for_result_inner, AppError::Internal),
                                            }
                                        });
                                    });

                                    share_window.present();
                                });

                                let runtime_for_delete = runtime_for_recv.clone();
                                let team_for_delete = Arc::clone(&team_for_recv);
                                let window_for_delete = window_for_recv.clone();
                                let refresh_after_delete = Rc::clone(&refresh_holder_for_rows_async);
                                delete_btn.connect_clicked(move |_| {
                                    let confirm = adw::MessageDialog::new(
                                        Some(&window_for_delete),
                                        Some(crate::tr!("manage-teams-delete-confirm-title").as_str()),
                                        Some(crate::tr!("manage-teams-delete-confirm-body").as_str()),
                                    );
                                    confirm.add_response("cancel", crate::tr!("common-cancel").as_str());
                                    confirm.add_response("delete", crate::tr!("manage-teams-delete-button").as_str());
                                    confirm.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
                                    let team_for_resp = Arc::clone(&team_for_delete);
                                    let runtime_for_resp = runtime_for_delete.clone();
                                    let window_for_resp = window_for_delete.clone();
                                    let refresh_after_resp = Rc::clone(&refresh_after_delete);
                                    confirm.connect_response(Some("delete"), move |d, _| {
                                        d.close();
                                        let (sender, receiver) = tokio::sync::oneshot::channel();
                                        let runtime_for_task = runtime_for_resp.clone();
                                        let team_for_task = Arc::clone(&team_for_resp);
                                        std::thread::spawn(move || {
                                            let result = runtime_for_task.block_on(async move {
                                                team_for_task.delete_team(actor_user_id, team_id).await
                                            });
                                            let _ = sender.send(result);
                                        });

                                        let window_for_result = window_for_resp.clone();
                                        let refresh_after_result = Rc::clone(&refresh_after_resp);
                                        glib::MainContext::default().spawn_local(async move {
                                            match receiver.await {
                                                Ok(Ok(())) => {
                                                    if let Some(refresh_cb) = refresh_after_result.borrow().as_ref() {
                                                        refresh_cb();
                                                    }
                                                }
                                                Ok(Err(err)) => Self::show_error(&window_for_result, err),
                                                Err(_) => Self::show_error(&window_for_result, AppError::Internal),
                                            }
                                        });
                                    });
                                    confirm.present();
                                });
                            }
                        }
                        Ok(Err(err)) => Self::show_error(&window_for_recv, err),
                        Err(_) => Self::show_error(&window_for_recv, AppError::Internal),
                    }
                });
            })
        };

        *refresh_holder.borrow_mut() = Some(refresh.clone());

        {
            let runtime_for_create = runtime_handle.clone();
            let team_for_create = Arc::clone(&team_service);
            let window_for_create = window.clone();
            let refresh_for_create = refresh.clone();
            create_btn.connect_clicked(move |_| {
                let create_window = gtk4::Window::builder()
                    .title(crate::tr!("manage-teams-create-window-title").as_str())
                    .modal(true)
                    .transient_for(&window_for_create)
                    .default_width(420)
                    .default_height(120)
                    .build();

                let root = gtk4::Box::builder()
                    .orientation(gtk4::Orientation::Vertical)
                    .spacing(8)
                    .margin_top(12)
                    .margin_bottom(12)
                    .margin_start(12)
                    .margin_end(12)
                    .build();
                let name_entry = gtk4::Entry::new();
                name_entry.set_placeholder_text(Some(
                    crate::tr!("manage-teams-name-placeholder").as_str(),
                ));
                let actions = gtk4::Box::builder()
                    .orientation(gtk4::Orientation::Horizontal)
                    .spacing(8)
                    .halign(gtk4::Align::End)
                    .build();
                let cancel = gtk4::Button::with_label(crate::tr!("common-cancel").as_str());
                let create = gtk4::Button::with_label(crate::tr!("manage-teams-create").as_str());
                create.add_css_class("suggested-action");
                actions.append(&cancel);
                actions.append(&create);
                root.append(&name_entry);
                root.append(&actions);
                create_window.set_child(Some(&root));

                let create_window_for_cancel = create_window.clone();
                cancel.connect_clicked(move |_| create_window_for_cancel.close());

                let runtime_for_apply = runtime_for_create.clone();
                let team_for_apply = Arc::clone(&team_for_create);
                let window_for_apply = window_for_create.clone();
                let refresh_after_apply = refresh_for_create.clone();
                let create_window_for_apply = create_window.clone();
                create.connect_clicked(move |_| {
                    let team_name = name_entry.text().to_string();
                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    let runtime_for_task = runtime_for_apply.clone();
                    let team_for_task = Arc::clone(&team_for_apply);
                    std::thread::spawn(move || {
                        let result = runtime_for_task.block_on(async move {
                            team_for_task
                                .create_team(actor_user_id, team_name.as_str())
                                .await
                        });
                        let _ = sender.send(result);
                    });

                    let window_for_result = window_for_apply.clone();
                    let refresh_after_result = refresh_after_apply.clone();
                    let create_window_close = create_window_for_apply.clone();
                    glib::MainContext::default().spawn_local(async move {
                        match receiver.await {
                            Ok(Ok(_)) => {
                                create_window_close.close();
                                refresh_after_result();
                            }
                            Ok(Err(err)) => Self::show_error(&window_for_result, err),
                            Err(_) => Self::show_error(&window_for_result, AppError::Internal),
                        }
                    });
                });

                create_window.present();
            });
        }

        refresh();

        Self { window }
    }

    fn show_warning(window: &gtk4::Window, title: &str, body: &str) {
        let dialog = adw::MessageDialog::new(Some(window), Some(title), Some(body));
        dialog.add_response("ok", crate::tr!("common-ok").as_str());
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("ok");
        dialog.present();
    }

    fn show_error(window: &gtk4::Window, err: AppError) {
        let title = match err {
            AppError::Authorization(_) => {
                crate::tr!("manage-teams-error-authorization").to_string()
            }
            _ => crate::tr!("manage-teams-error-generic").to_string(),
        };
        let body = err.to_string();
        let dialog =
            adw::MessageDialog::new(Some(window), Some(title.as_str()), Some(body.as_str()));
        dialog.add_response("ok", crate::tr!("common-ok").as_str());
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("ok");
        dialog.present();
    }

    pub fn take_content(&self) -> Option<gtk4::Widget> {
        let child = self.window.child();
        if child.is_some() {
            self.window.set_child(Option::<&gtk4::Widget>::None);
        }
        child
    }
}
