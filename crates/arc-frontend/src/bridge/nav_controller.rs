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
        #[cxx_name = "openChild"]
        fn open_child(self: Pin<&mut NavController>, entry: QString);

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
        Some(e) if e.kind == "category" => "search".to_string(),
        Some(e) => e.kind.clone(),
        None => current_tab.kind.clone(),
    }
}

#[derive(Default)]
pub struct NavControllerRust {
    stack: Vec<Entry>,
    forward: Vec<Entry>,
    current_tab: Entry,
    can_go_back: bool,
    can_go_forward: bool,
    current_view: QString,
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

    pub fn navigate(mut self: Pin<&mut Self>, spec: QString) {
        let Some(target) = serde_json::from_str::<Vec<Entry>>(&spec.to_string())
            .ok()
            .and_then(|v| v.into_iter().next())
        else {
            return;
        };

        if is_tab_kind(&target.kind) {
            if target.same(&self.current_tab) && self.stack.is_empty() {
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
            self.as_mut().emit_op(serde_json::json!({ "action": "popTo", "depth": i + 1 }));
        } else {
            self.as_mut().rust_mut().stack.push(target.clone());
            self.as_mut().rust_mut().forward.clear();
            self.as_mut().emit_op(serde_json::json!({ "action": "push", "entry": target }));
        }
        self.sync();
    }

    pub fn open_child(mut self: Pin<&mut Self>, entry: QString) {
        let Ok(entry) = serde_json::from_str::<Entry>(&entry.to_string()) else {
            return;
        };
        self.as_mut().rust_mut().stack.push(entry.clone());
        self.as_mut().rust_mut().forward.clear();
        self.as_mut().emit_op(serde_json::json!({ "action": "push", "entry": entry }));
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
        self.as_mut().emit_op(serde_json::json!({ "action": "pop" }));
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
