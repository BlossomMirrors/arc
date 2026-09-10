#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, can_go_back, cxx_name = "canGoBack")]
        #[qproperty(bool, can_go_forward, cxx_name = "canGoForward")]
        #[qproperty(QString, current_view, cxx_name = "currentView")]
        type NavController = super::NavControllerRust;
        #[qinvokable]
        fn navigate(self: Pin<&mut NavController>, spec: QString);

        #[qinvokable]
        #[cxx_name = "openApp"]
        fn open_app(self: Pin<&mut NavController>, pkg_id: QString, seed_json: QString);

        #[qinvokable]
        #[cxx_name = "openStory"]
        fn open_story(self: Pin<&mut NavController>, story_id: QString);

        #[qinvokable]
        #[cxx_name = "openCategory"]
        fn open_category(
            self: Pin<&mut NavController>,
            id: QString,
            label: QString,
            color: QString,
            icon: QString,
        );

        #[qinvokable]
        #[cxx_name = "openList"]
        fn open_list(self: Pin<&mut NavController>, slug: QString, label: QString);

        #[qinvokable]
        #[cxx_name = "goHome"]
        fn go_home(self: Pin<&mut NavController>);

        #[qinvokable]
        #[cxx_name = "goSearch"]
        fn go_search(self: Pin<&mut NavController>, query: QString);

        #[qinvokable]
        #[cxx_name = "goInstalled"]
        fn go_installed(self: Pin<&mut NavController>);

        #[qinvokable]
        #[cxx_name = "goDownloads"]
        fn go_downloads(self: Pin<&mut NavController>);

        #[qinvokable]
        #[cxx_name = "goSettings"]
        fn go_settings(self: Pin<&mut NavController>);

        #[qinvokable]
        #[cxx_name = "updateQuery"]
        fn update_query(self: Pin<&mut NavController>, query: QString);

        #[qinvokable]
        #[cxx_name = "goBack"]
        fn go_back(self: Pin<&mut NavController>);

        #[qinvokable]
        #[cxx_name = "goForward"]
        fn go_forward(self: Pin<&mut NavController>);

        #[qsignal]
        #[cxx_name = "navOp"]
        fn nav_op(self: Pin<&mut NavController>, op: QString);
    }
}

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::pin::Pin;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Entry {
    kind: String,
    #[serde(default)]
    a: String,
    #[serde(default)]
    b: String,
    #[serde(default)]
    c: serde_json::Value,
    #[serde(default)]
    d: String,
}

impl Entry {
    fn same(&self, other: &Entry) -> bool {
        self.kind == other.kind && self.a == other.a && self.b == other.b
    }
}

fn is_tab_kind(kind: &str) -> bool {
    matches!(kind, "home" | "search" | "installed" | "downloads" | "settings")
}

fn view_of(stack: &[Entry], current_tab: &Entry) -> String {
    match stack.last() {
        Some(e) if e.kind == "category" || e.kind == "list" => "search".to_string(),
        Some(e) => e.kind.clone(),
        None => current_tab.kind.clone(),
    }
}

pub struct NavControllerRust {
    stack: Vec<Entry>,
    forward: Vec<Entry>,
    current_tab: Entry,
    can_go_back: bool,
    can_go_forward: bool,
    current_view: QString,
}

impl Default for NavControllerRust {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            forward: Vec::new(),
            current_tab: Entry::default(),
            can_go_back: false,
            can_go_forward: false,
            current_view: QString::from("home"),
        }
    }
}

impl Default for Entry {
    fn default() -> Self {
        Entry {
            kind: "home".to_string(),
            a: String::new(),
            b: String::new(),
            c: serde_json::Value::Null,
            d: String::new(),
        }
    }
}

impl qobject::NavController {
    fn sync(mut self: Pin<&mut Self>) {
        let back = !self.stack.is_empty();
        let forward = !self.forward.is_empty();
        let view = view_of(&self.stack, &self.current_tab);
        self.as_mut().set_can_go_back(back);
        self.as_mut().set_can_go_forward(forward);
        self.as_mut().set_current_view(QString::from(&view));
    }

    fn emit_op(self: Pin<&mut Self>, op: serde_json::Value) {
        self.nav_op(QString::from(&op.to_string()));
    }

    pub fn navigate(self: Pin<&mut Self>, spec: QString) {
        let Some(target) = serde_json::from_str::<Vec<Entry>>(&spec.to_string())
            .ok()
            .and_then(|v| v.into_iter().next())
        else {
            return;
        };
        self.go(target);
    }

    pub fn open_app(self: Pin<&mut Self>, pkg_id: QString, seed_json: QString) {
        let seed = serde_json::from_str(&seed_json.to_string()).unwrap_or(serde_json::Value::Null);
        self.go(Entry {
            kind: "detail".to_string(),
            a: pkg_id.to_string(),
            c: seed,
            ..Default::default()
        });
    }

    pub fn open_story(self: Pin<&mut Self>, story_id: QString) {
        self.go(Entry { kind: "story".to_string(), a: story_id.to_string(), ..Default::default() });
    }

    pub fn open_category(
        self: Pin<&mut Self>,
        id: QString,
        label: QString,
        color: QString,
        icon: QString,
    ) {
        self.go(Entry {
            kind: "category".to_string(),
            a: id.to_string(),
            b: label.to_string(),
            c: serde_json::Value::String(color.to_string()),
            d: icon.to_string(),
        });
    }

    pub fn open_list(self: Pin<&mut Self>, slug: QString, label: QString) {
        self.go(Entry {
            kind: "list".to_string(),
            a: slug.to_string(),
            b: label.to_string(),
            ..Default::default()
        });
    }

    pub fn go_home(self: Pin<&mut Self>) {
        self.go(Entry { kind: "home".to_string(), ..Default::default() });
    }

    pub fn go_search(self: Pin<&mut Self>, query: QString) {
        self.go(Entry {
            kind: "search".to_string(),
            a: query.to_string(),
            ..Default::default()
        });
    }

    pub fn go_installed(self: Pin<&mut Self>) {
        self.go(Entry { kind: "installed".to_string(), ..Default::default() });
    }

    pub fn go_downloads(self: Pin<&mut Self>) {
        self.go(Entry { kind: "downloads".to_string(), ..Default::default() });
    }

    pub fn go_settings(self: Pin<&mut Self>) {
        self.go(Entry { kind: "settings".to_string(), ..Default::default() });
    }

    fn go(mut self: Pin<&mut Self>, target: Entry) {
        if is_tab_kind(&target.kind) {
            if target.same(&self.current_tab) && self.stack.is_empty() {
                self.sync();
                return;
            }
            self.as_mut().rust_mut().current_tab = target.clone();
            self.as_mut().rust_mut().stack.clear();
            self.as_mut().rust_mut().forward.clear();
            self.as_mut().emit_op(serde_json::json!({ "action": "tab", "entry": target }));
        } else if let Some(i) = self.stack.iter().position(|e| e.same(&target)) {
            if i + 1 == self.stack.len() {
                return;
            }
            self.as_mut().rust_mut().stack.truncate(i + 1);
            self.as_mut().rust_mut().forward.clear();
            let top = self.stack.last().cloned();
            self.as_mut()
                .emit_op(serde_json::json!({ "action": "popTo", "depth": i + 1, "top": top }));
        } else {
            self.as_mut().rust_mut().stack.push(target.clone());
            self.as_mut().rust_mut().forward.clear();
            self.as_mut().emit_op(serde_json::json!({ "action": "push", "entry": target }));
        }
        self.sync();
    }

    pub fn update_query(mut self: Pin<&mut Self>, query: QString) {
        let query = query.to_string();
        if !self.stack.is_empty() || self.current_tab.kind != "search" {
            return;
        }
        self.as_mut().rust_mut().current_tab.a = query;
    }

    pub fn go_back(mut self: Pin<&mut Self>) {
        if self.stack.is_empty() {
            return;
        }
        let popped = self.as_mut().rust_mut().stack.pop().unwrap();
        self.as_mut().rust_mut().forward.push(popped);

        let top = self.stack.last().cloned();
        self.as_mut().emit_op(serde_json::json!({ "action": "pop", "top": top }));
        self.sync();
    }

    pub fn go_forward(mut self: Pin<&mut Self>) {
        let Some(entry) = self.as_mut().rust_mut().forward.pop() else {
            return;
        };
        self.as_mut().rust_mut().stack.push(entry.clone());
        self.as_mut().emit_op(serde_json::json!({ "action": "push", "entry": entry }));
        self.sync();
    }
}
