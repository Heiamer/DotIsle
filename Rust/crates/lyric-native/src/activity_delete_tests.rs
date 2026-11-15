use super::*;

struct Fixture { app: App, directory: PathBuf }
impl Fixture {
    fn new() -> Self {
        let directory=std::env::temp_dir().join(format!("dotisle-delete-card-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut app=App::new(PathBuf::new(),directory.clone(),true);
        app.todos=todos::Store::new(directory.join("todos.json"));
        app.timers=timers::Store::new(directory.join("timers.json"));
        Self{app,directory}
    }
    fn plugin(&mut self) {
        let folder=self.directory.join("package");std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("manifest.json"),json!({"apiVersion":1,"id":"example.progress","name":"创作进度","version":"1","views":{"island":[{"kind":"metric","text":"16","label":"今日完成"}]}}).to_string()).unwrap();
        self.app.plugins.install(&folder).unwrap();
        self.app.refresh_components();
        self.show("plugin:example.progress");
    }
    fn show(&mut self,id:&str) {
        self.app.selected=id.into();self.app.refresh_components();
        self.app.expanded=true;self.app.held=true;self.app.opacity=1.;
        self.app.frame=Frame::expanded();self.app.transition=None;
    }
}
impl Drop for Fixture { fn drop(&mut self) { let _=std::fs::remove_dir_all(&self.directory); } }

#[test]
fn delete_targets_work_in_classic_and_mini_without_stealing_content_or_gestures() {
    let mut fixture=Fixture::new();fixture.plugin();
    for mode in ["Classic","Mini"] { for width in [340.,400.,592.] {
        fixture.app.settings["IslandSizeMode"]=json!(mode);
        for (id,kind,close,hit) in [
            ("timer:fixture",1,layout::activity_close(width),layout::ACTIVITY_CLOSE),
            ("todos",2,layout::activity_close(width),layout::ACTIVITY_CLOSE),
            ("plugin:example.progress",4,layout::activity_close(width),layout::ACTIVITY_CLOSE),
            ("update",3,layout::update_close(width),51),
        ] {
            fixture.app.selected=id.into();
            fixture.app.panel=Some(activities::Panel{kind,..Default::default()});
            fixture.app.updates.state=json!({"phase":"applying"});
            fixture.app.layout.card=Rect::new(24.,24.,width,220.);
            for reveal in [0.8,1.] {
                fixture.app.frame=Frame::expanded();fixture.app.frame.card_reveal=reveal;
                let (shape,_)=fixture.app.shape();let scale=(shape.w/width).min(shape.h/220.);
                let x=shape.x+(close.x+close.w/2.)*scale;
                let y=shape.y+(close.y+close.h/2.+fixture.app.frame.content_offset_y)*scale;
                assert_eq!(fixture.app.hit(x,y),hit,"{mode} {width} {id} {reveal}");
                assert!(!fixture.app.can_swipe(hit));assert!(!fixture.app.can_hide(hit));
            }
            let panel=fixture.app.panel.as_ref();
            assert_eq!(layout::mini_hit(width,220.,panel,close.x+close.w/2.,close.y+close.h/2.),hit);
        }
    }}
    fixture.app.selected="music".into();fixture.app.panel=None;fixture.app.frame=Frame::expanded();
    let c=fixture.app.layout.card;let r=layout::activity_close(c.w);
    assert_ne!(fixture.app.hit(c.x+r.x+r.w/2.,c.y+r.y+r.h/2.),layout::ACTIVITY_CLOSE);
}

#[test]
fn clicking_x_deletes_only_the_selected_timer_and_plugin_registration_stays_deleted() {
    let mut fixture=Fixture::new();
    let first=fixture.app.timers.create(&json!({"mode":"stopwatch"})).unwrap();
    let second=fixture.app.timers.create(&json!({"mode":"stopwatch"})).unwrap();
    fixture.show(&format!("timer:{first}"));
    unsafe { fixture.app.activate(layout::ACTIVITY_CLOSE); }
    assert_eq!(fixture.app.timers.items.len(),1);assert_eq!(fixture.app.timers.items[0].id,second);
    assert!(!fixture.app.components.contains(&format!("timer:{first}")));
    fixture.plugin();
    unsafe { fixture.app.activate(layout::ACTIVITY_CLOSE); }
    assert!(fixture.app.plugins.get("example.progress").is_none());
    assert!(fixture.directory.join("package/manifest.json").is_file());
    fixture.app.poll();fixture.app.refresh_components();
    assert!(!fixture.app.expanded);
    assert!(!fixture.app.components.iter().any(|id|id.starts_with("plugin:")));
    let registry=dotisle_domain::plugins::Registry::new(fixture.directory.join("plugins.json"));
    assert!(registry.island_ids().is_empty());
}

#[test]
fn deleting_the_todo_card_preserves_tasks_and_failed_plugin_deletion_preserves_the_card() {
    let mut fixture=Fixture::new();
    fixture.app.todos.save(&json!({"title":"保留这个任务","date":fixture.app.todo_now.date,"time":"23:59"})).unwrap();
    fixture.show("todos");let before=fixture.app.todos.list(&fixture.app.todo_now);
    unsafe { fixture.app.activate(layout::ACTIVITY_CLOSE); }
    assert_eq!(fixture.app.todos.list(&fixture.app.todo_now),before);
    assert!(!fixture.app.flag("TodoIslandEnabled",true));
    fixture.app.poll();fixture.app.refresh_components();
    assert!(!fixture.app.components.iter().any(|id|id=="todos"));
    fixture.plugin();fixture.app.plugins.error="模拟注册表写入失败".into();
    assert!(fixture.app.delete_activity().is_err());
    assert!(fixture.app.expanded);assert!(fixture.app.plugins.get("example.progress").is_some());
}

#[test]
fn deleting_update_cards_keeps_the_download_and_install_action_and_can_be_reopened_explicitly() {
    let mut fixture=Fixture::new();
    for phase in ["available","checking","downloading","verifying","ready","applying","restarting","completed","failed"] {
        fixture.app.update_card_deleted=false;
        fixture.app.updates.state=json!({"phase":phase,"version":"5.0.0-test","receivedBytes":128});
        fixture.app.updates.action=if phase=="applying"{"apply"}else{""}.into();
        fixture.show("update");let state=fixture.app.updates.state.clone();let action=fixture.app.updates.action.clone();
        unsafe { fixture.app.activate(51); }
        assert_eq!(fixture.app.updates.state,state);assert_eq!(fixture.app.updates.action,action);
        fixture.app.poll();fixture.app.refresh_components();
        assert!(!fixture.app.expanded);assert!(!fixture.app.components.iter().any(|id|id=="update"));
        let reply=unsafe { fixture.app.request(json!({"version":1,"command":"showUpdate"})) };
        assert_eq!(reply["success"],true);assert!(fixture.app.expanded);assert_eq!(fixture.app.selected,"update");
    }
}

#[test]
fn deletion_cannot_hover_open_the_next_card_during_the_closing_morph() {
    let mut fixture=Fixture::new();fixture.plugin();
    fixture.app.settings["HoverExpand"]=json!(true);fixture.app.settings["Animation"]=json!(true);
    fixture.app.delete_activity().unwrap();assert!(fixture.app.transition.is_some());
    fixture.app.track_pointer_presence(false);fixture.app.track_pointer_presence(true);
    assert!(!fixture.app.expanded);
    fixture.app.transition=None;fixture.app.track_pointer_presence(true);
    assert!(!fixture.app.expanded);
    fixture.app.track_pointer_presence(false);fixture.app.track_pointer_presence(true);
    assert!(fixture.app.expanded);
}
