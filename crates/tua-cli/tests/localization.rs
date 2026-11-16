use std::fs;
use std::process::Command;

mod support;

fn configured_project(name: &str) -> support::TempDir {
    let project = support::TempDir::new(name);
    fs::create_dir_all(project.path().join("src/locales")).unwrap();
    fs::write(project.path().join("tua.toml"), "source = 'src'\nout = 'build'\n[i18n]\nenabled = true\ndefault_locale = 'en'\nfallback_locale = 'en'\n[[i18n.catalogs]]\nlocale = 'en'\npath = 'locales/en.lua'\n[[i18n.catalogs]]\nlocale = 'fr'\npath = 'locales/fr.lua'\n").unwrap();
    fs::write(project.path().join("src/locales/en.lua"), "return { en = { menu = { play = 'Play %{name}' }, coins = { one = '%{count} coin', other = '%{count} coins' } } }\n").unwrap();
    fs::write(project.path().join("src/locales/fr.lua"), "return { fr = { menu = { play = 'Jouer %{name}' }, coins = { one = '%{count} pièce', other = '%{count} pièces' } } }\n").unwrap();
    project
}

fn run(project: &support::TempDir, command: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tua"))
        .arg(command)
        .current_dir(project.path())
        .output()
        .unwrap()
}

#[test]
fn localization_build_is_lazy_and_runtime_is_shared_across_modules() {
    let project = configured_project("localization-build");
    fs::write(
        project.path().join("src/other.tua"),
        "return function() return I18n.getLocale() end\n",
    )
    .unwrap();
    fs::write(project.path().join("src/main.tua"), "local getLocale = require('other')\nI18n.setLocale('fr')\nassert(getLocale() == 'fr')\nassert(I18n.t('menu.play', { name = 'Ada' }) == 'Jouer Ada')\nassert(I18n.t('coins', { count = 2 }) == '2 pièces')\npackage.loaded.tuai18n = nil\nassert(require('tuai18n').getLocale() == 'fr')\nprint('localization-ok')\n").unwrap();
    let built = run(&project, "build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let out = project.path().join("build");
    for file in [
        "tuai18n.lua",
        "tuai18n_config.lua",
        "smiti18n/init.lua",
        "smiti18n/LICENSE",
        "locales/en.lua",
        "locales/fr.lua",
    ] {
        assert!(out.join(file).is_file(), "missing {file}");
    }
    assert!(fs::read_to_string(out.join("main.lua"))
        .unwrap()
        .contains("local I18n = require(\"tuai18n\")"));
    if let Some(lua) = support::lua_executable() {
        let result = Command::new(lua)
            .arg("main.lua")
            .current_dir(&out)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).trim(),
            "localization-ok"
        );
    }

    fs::write(project.path().join("src/main.tua"), "print('unused')\n").unwrap();
    fs::write(
        project.path().join("src/other.tua"),
        "return function() return 'unused' end\n",
    )
    .unwrap();
    let built = run(&project, "build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(!out.join("tuai18n.lua").exists());
    assert!(!out.join("tuai18n_config.lua").exists());
    assert!(!out.join("smiti18n/init.lua").exists());
}

#[test]
fn localization_runtime_follows_module_values_but_not_matching_property_names() {
    let project = configured_project("localization-module-value");
    fs::write(
        project.path().join("src/main.tua"),
        "local translator = I18n\nassert(translator.t('menu.play', {name = 'Ada'}) == 'Play Ada')\n",
    )
    .unwrap();
    let built = run(&project, "build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let out = project.path().join("build");
    assert!(fs::read_to_string(out.join("main.lua"))
        .unwrap()
        .starts_with("local I18n = require(\"tuai18n\")\n"));
    if let Some(lua) = support::lua_executable() {
        let result = Command::new(lua)
            .arg("main.lua")
            .current_dir(&out)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    fs::write(
        project.path().join("src/main.tua"),
        "local metadata = {I18n = {value = 3}}\nassert(metadata.I18n.value == 3)\n",
    )
    .unwrap();
    let built = run(&project, "build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(!fs::read_to_string(out.join("main.lua"))
        .unwrap()
        .contains("tuai18n"));
    assert!(!out.join("tuai18n.lua").exists());
}

#[test]
fn localization_check_reports_catalog_errors_without_executing_catalogs() {
    let project = configured_project("localization-catalog-errors");
    fs::write(
        project.path().join("src/main.tua"),
        "print(I18n.t('menu.play'))\n",
    )
    .unwrap();
    fs::write(
        project.path().join("src/locales/fr.lua"),
        "return { wrong = { menu = { play = os.execute('exit 99') } } }\n",
    )
    .unwrap();
    let checked = run(&project, "check");
    assert!(!checked.status.success());
    assert!(
        String::from_utf8_lossy(&checked.stderr).contains("catalog root"),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    fs::remove_file(project.path().join("src/locales/fr.lua")).unwrap();
    let checked = run(&project, "check");
    assert!(!checked.status.success());
    assert!(
        String::from_utf8_lossy(&checked.stderr).contains("catalog is missing"),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
}

#[test]
fn localization_builtin_yields_to_local_binding_and_is_absent_when_disabled() {
    let project = configured_project("localization-shadow");
    fs::write(project.path().join("src/main.tua"), "local function read()\n  local I18n = { t = function(_) return 'custom' end }\n  return I18n.t('own.key')\nend\nassert(read() == 'custom')\n").unwrap();
    let built = run(&project, "build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(!project.path().join("build/tuai18n.lua").exists());
    fs::write(
        project.path().join("tua.toml"),
        "source = 'src'\nout = 'build'\n[i18n]\nenabled = false\n",
    )
    .unwrap();
    fs::write(
        project.path().join("src/main.tua"),
        "local I18n = require('custom_i18n')\nprint(I18n.t('own.key'))\n",
    )
    .unwrap();
    fs::write(
        project.path().join("src/custom_i18n.lua"),
        "return { t = function(_) return 'custom' end }\n",
    )
    .unwrap();
    let built = run(&project, "build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(!project.path().join("build/tuai18n.lua").exists());
    assert!(project.path().join("build/custom_i18n.lua").exists());
}

#[test]
fn localization_runtime_handles_parent_locale_plurals_dialogue_and_advanced_fields() {
    let project = support::TempDir::new("localization-runtime-forms");
    fs::create_dir_all(project.path().join("src/locales")).unwrap();
    fs::write(project.path().join("tua.toml"), "source = 'src'\nout = 'build'\n[i18n]\nenabled = true\ndefault_locale = 'en'\nfallback_locale = 'en'\n[[i18n.catalogs]]\nlocale = 'en'\npath = 'locales/en.lua'\n[[i18n.catalogs]]\nlocale = 'fr'\npath = 'locales/fr.lua'\n[[i18n.catalogs]]\nlocale = 'ru'\npath = 'locales/ru.lua'\n").unwrap();
    fs::write(project.path().join("src/locales/en.lua"), "return { en = { menu = { play = 'Play' }, coins = { one = '%{count} coin', other = '%{count} coins' }, dialogue = { 'Hi %{name}', 'Bye %{name}' }, score = '%<count>.d points', dynamic = tostring(42) } }\n").unwrap();
    fs::write(
        project.path().join("src/locales/fr.lua"),
        "return { fr = { menu = { play = 'Jouer' } } }\n",
    )
    .unwrap();
    fs::write(project.path().join("src/locales/ru.lua"), "return { ru = { coins = { one = '%{count} монета', few = '%{count} монеты', many = '%{count} монет', other = '%{count} монеты' } } }\n").unwrap();
    fs::write(project.path().join("src/main.tua"), "assert(I18n.t('menu.play', {}, 'fr-CA') == 'Jouer')\nI18n.setLocale('fr')\nassert(I18n.t('menu.play', {}, 'ru') == 'Play')\nassert(I18n.t('menu.play', { locale = 'fr' }, 'ru') == 'Play')\nassert(I18n.getLocale() == 'fr')\nI18n.setLocale('en')\nassert(I18n.t('coins', { count = 2 }, 'ru') == '2 монеты')\nassert(I18n.t('coins', { count = 5 }, 'ru') == '5 монет')\nlocal lines: any = I18n.t('dialogue', { name = 'Ada' })\nassert(lines[1] == 'Hi Ada' and lines[2] == 'Bye Ada')\nassert(I18n.t('score', { count = 7 }) == '7 points')\nassert(I18n.t('dynamic') == '42')\nassert(type(I18n.formatNumber(1000)) == 'string')\nassert(type(I18n.formatPrice(2)) == 'string')\nassert(type(I18n.formatDate()) == 'string')\nprint('forms-ok')\n").unwrap();
    let built = run(&project, "build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    if let Some(lua) = support::lua_executable() {
        let result = Command::new(lua)
            .arg("main.lua")
            .current_dir(project.path().join("build"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "forms-ok");
    }
}
