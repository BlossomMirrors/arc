use crate::AppWindow;

pub fn apply_translations(app: &AppWindow) {
    use tr::tr;
    app.set_tr_install(tr!("Install").into());
    app.set_tr_remove(tr!("Remove").into());
    app.set_tr_update(tr!("Update").into());
    app.set_tr_cancel(tr!("Cancel").into());
    app.set_tr_loading(tr!("Loading...").into());
    app.set_tr_installed(tr!("Installed").into());
    app.set_tr_back(tr!("Back").into());
    app.set_tr_run(tr!("Run").into());
    app.set_tr_working(tr!("Working...").into());
    app.set_tr_verified(tr!("Verified developer").into());
    app.set_tr_website(tr!("Website").into());
    app.set_tr_extensions(tr!("Extensions").into());
    app.set_tr_restart(tr!("Restart").into());
    app.set_tr_name(tr!("Name").into());
    app.set_tr_url(tr!("URL").into());
    app.set_tr_categories(tr!("Categories").into());
    app.set_tr_recommended_apps(tr!("Recommended Apps").into());
    app.set_tr_no_packages(
        tr!("No packages found.\nSearch for an application to get started.").into(),
    );
    app.set_tr_up_to_date(tr!("Everything is up to date.").into());
    app.set_tr_no_installed(tr!("No applications installed.").into());
    app.set_tr_downloads(tr!("Downloads & Updates").into());
    app.set_tr_checking(tr!("Checking...").into());
    app.set_tr_check_updates(tr!("Check for Updates").into());
    app.set_tr_active_prefix(tr!("Active (").into());
    app.set_tr_pending_prefix(tr!("Pending (").into());
    app.set_tr_completed_prefix(tr!("Completed (").into());
    app.set_tr_clear_all(tr!("Clear All").into());
    app.set_tr_updates_available_prefix(tr!("Updates Available (").into());
    app.set_tr_update_all(tr!("Update All").into());
    app.set_tr_no_downloads(tr!("No active downloads").into());
    app.set_tr_downloads_empty(tr!("Installs and updates will appear here").into());
    app.set_tr_removed(tr!("Removed").into());
    app.set_tr_failed(tr!("Failed").into());
    app.set_tr_queued(tr!("Queued").into());
    app.set_tr_removing(tr!("Removing").into());
    app.set_tr_updating(tr!("Updating").into());
    app.set_tr_installing(tr!("Installing").into());
    app.set_tr_license(tr!("License Agreement").into());
    app.set_tr_eula_requires(
        tr!(" requires you to accept its End User License Agreement before installing.").into(),
    );
    app.set_tr_read_license(tr!("Read License Agreement →").into());
    app.set_tr_accept_install(tr!("Accept & Install").into());
    app.set_tr_install_flatpak_bundle(tr!("Install Flatpak Bundle").into());
    app.set_tr_install_flatpak_bundle_desc(
        tr!("This will install the bundled Flatpak application on your system.").into(),
    );
    app.set_tr_install_appimage(tr!("Install AppImage").into());
    app.set_tr_install_appimage_desc(
        tr!("This will register the AppImage so it appears in your app launcher.").into(),
    );
    app.set_tr_install_package(tr!("Install Package").into());
    app.set_tr_install_package_desc(
        tr!("Choose how you'd like to install this package file.").into(),
    );
    app.set_tr_install_bundle(tr!("Install Bundle").into());
    app.set_tr_install_as_appimage(tr!("Install as AppImage").into());
    app.set_tr_install_distrobox(tr!("Install via Distrobox").into());
    app.set_tr_searching_flathub(tr!("Searching Flathub for an alternative...").into());
    app.set_tr_file_security(tr!("Third-party software has broad access to system resources and may pose a security risk. Only install files from sources you trust.").into());
    app.set_tr_add_repo(tr!("Add Repository").into());
    app.set_tr_unknown_repo(tr!("Unknown repository").into());
    app.set_tr_repo_add_desc(tr!("This will add the repository to your Flatpak user installation and make its applications available to install.").into());
    app.set_tr_repo_warning(tr!("Adding a third-party repository gives it the ability to distribute software to your system. Only add repositories from sources you trust.").into());
    app.set_tr_contains_repo(tr!("Contains repository").into());
    app.set_tr_install_application(tr!("Install Application").into());
    app.set_tr_settings(tr!("Settings").into());
    app.set_tr_general(tr!("GENERAL").into());
    app.set_tr_auto_updates(tr!("Auto-updates").into());
    app.set_tr_auto_updates_desc(
        tr!("Automatically update installed apps in the background").into(),
    );
    app.set_tr_security_warnings(tr!("Security warnings").into());
    app.set_tr_security_warnings_desc(
        tr!("Show warnings when installing third-party software or adding repositories").into(),
    );
    app.set_tr_downloads_section(tr!("DOWNLOADS").into());
    app.set_tr_concurrent(tr!("Concurrent downloads").into());
    app.set_tr_concurrent_desc(tr!("Maximum number of simultaneous downloads (1-16)").into());
    app.set_tr_repositories(tr!("REPOSITORIES").into());
    app.set_tr_loading_ellipsis(tr!("Loading…").into());
    app.set_tr_protected(tr!("Protected").into());
    app.set_tr_add_repo_section(tr!("ADD REPOSITORY").into());
    app.set_tr_danger_zone(tr!("DANGER ZONE").into());
    app.set_tr_force_updates(tr!("Force updates").into());
    app.set_tr_force_updates_desc(
        tr!("Runs flatpak update -y directly, bypassing the daemon").into(),
    );
    app.set_tr_force_update(tr!("Force Update").into());
    app.set_tr_restart_daemon(tr!("Restart daemon").into());
    app.set_tr_restart_daemon_desc(
        tr!("Kills the running arc-daemon and starts a fresh one in the background").into(),
    );
    app.set_tr_search_placeholder(tr!("Search for applications...").into());
    app.set_tr_proprietary(tr!("Proprietary").into());
    app.set_tr_all_ages(tr!("All ages").into());
    app.set_tr_pwa(tr!("PWA").into());
    app.set_tr_uninstall(tr!("Uninstall").into());
    app.set_tr_install_from_suffix(tr!(" from Arc Store instead").into());
    // Verb-final languages (German, etc.) translate this sentinel to a non-empty value.
    // When detected, clear the verb prefix so the name leads and the suffix carries the verb.
    if tr!("verb_final_word_order") != "verb_final_word_order" {
        app.set_tr_install_from_prefix("".into());
        app.set_tr_uninstall_title_prefix("".into());
        app.set_tr_uninstall_title_suffix(tr!("uninstall_suffix").into());
    }
    app.set_tr_delete_data(tr!("Also delete app data").into());
    app.set_tr_delete_data_desc(tr!("Removes settings and saved data").into());
}
