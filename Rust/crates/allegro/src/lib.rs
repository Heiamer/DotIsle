//! Allegro — 点屿下一代桌搭与效率小组件引擎。
//! 集成五套材质/皮肤体系、壁纸局部亚克力磨砂、跨组件磁吸辅助线、智能叠放 (Smart Stack)、
//! `.dotlayout` 预设分享、热键浮层仪表盘、全屏游戏休眠、GPU/网速监控以及单显示器全屏合批透明画布引擎。
pub mod canvas;
pub mod efficiency;
pub mod layout_pack;
pub mod snap;
pub mod stack;
pub mod stage;
pub mod telemetry;
pub mod theme;
pub mod wallpaper;

#[cfg(test)]
mod tests {
    use super::*;
    use dotisle_domain::desktop_widgets::{Kind, Placement, Size, Style};

    fn sample_placement(id: &str, kind: Kind, x: i32, y: i32) -> Placement {
        Placement {
            id: id.into(),
            kind,
            size: Size::Medium,
            style: Style::Original,
            config: Default::default(),
            x,
            y,
            width: 0.0,
            height: 0.0,
            hidden: false,
            revision: 1,
            material: String::new(),
            opacity: 0.0,
            stack_id: String::new(),
            stack_order: 0,
        }
    }

    #[test]
    fn theme_and_material_cycle_cover_two_variants_with_legacy_fallback() {
        let mut mat = theme::WidgetMaterial::Lumen;
        let mut seen = Vec::new();
        for _ in 0..2 {
            seen.push(mat);
            mat = mat.next();
        }
        assert_eq!(mat, theme::WidgetMaterial::Lumen);
        assert_eq!(seen.len(), 2);
        assert_eq!(theme::WidgetMaterial::default(), theme::WidgetMaterial::Acrylic);
        assert_eq!(theme::WidgetMaterial::from_str_opt(""), theme::WidgetMaterial::Acrylic);
        assert_eq!(theme::WidgetMaterial::from_str_opt("acrylic"), theme::WidgetMaterial::Acrylic);
        assert_eq!(theme::WidgetMaterial::from_str_opt("liquidGlass"), theme::WidgetMaterial::Acrylic);
        assert_eq!(theme::WidgetMaterial::from_str_opt("transparent"), theme::WidgetMaterial::Acrylic);
        assert_eq!(theme::WidgetMaterial::from_str_opt("lumen"), theme::WidgetMaterial::Lumen);
        assert_eq!(theme::parse_hex_rgb("#389bff"), Some(0x389BFF));
        assert_eq!(theme::parse_hex_rgb("bad"), None);

        let mut stack_scene = vello::Scene::new();
        stack::paint_stack_depth_layers(&mut stack_scene, snap::Rect2D::new(0.0, 0.0, 368.0, 176.0), 3, false);
        theme::paint_micro_dot_grid(&mut stack_scene, vello::kurbo::Rect::new(0.0, 0.0, 368.0, 176.0), 24.0, true);

        for style in Style::ALL {
            let t_light = theme::resolve_theme(style, false, None);
            let t_dark = theme::resolve_theme(style, true, Some(0x123456));
            assert!(t_light.radius > 0.0);
            assert_eq!(t_dark.primary, 0x123456);
        }
    }

    #[test]
    fn paint_shell_and_acrylic_wallpaper_work() {
        let mut wall = wallpaper::WallpaperCache::new();
        let mut buf = vec![0u8; 64 * 64 * 4];
        for y in 0..64u32 {
            for x in 0..64u32 {
                let i = ((y * 64 + x) * 4) as usize;
                buf[i] = (x * 4) as u8;
                buf[i + 1] = (y * 4) as u8;
                buf[i + 2] = 200;
                buf[i + 3] = 255;
            }
        }
        wall.set_synthetic_wallpaper(64, 64, buf);
        let acrylic = wall
            .crop_acrylic("m1", (1920.0, 1080.0), snap::Rect2D::new(40.0, 40.0, 368.0, 176.0))
            .expect("synthetic acrylic crop");
        assert!(acrylic.image.width >= 24 && acrylic.image.height >= 24);
        assert!(acrylic.avg_luma > 0.0 && acrylic.avg_luma <= 1.0);

        for mat in theme::WidgetMaterial::ALL {
            let mut scene = vello::Scene::new();
            let tokens = theme::resolve_theme(Style::Original, true, None);
            let app = theme::WidgetAppearance {
                material: mat,
                style: Style::Original,
                opacity: 0.92,
                radius: 24.0,
                accent: Some(0x389BFF),
                blur_wallpaper: true,
            };
            theme::paint_shell(
                &mut scene,
                vello::kurbo::Rect::new(0., 0., 368., 176.),
                app,
                tokens,
                tokens.primary,
                true,
                Some(&acrylic.image),
            );
            assert!(!scene.is_empty());
        }
    }

    #[test]
    fn snap_drag_aligns_to_peer_edge_gap_and_detects_stack_target() {
        let screen = snap::Rect2D::new(0., 0., 1920., 1080.);
        let peers = vec![snap::SnapPeer {
            id: "a".into(),
            rect: snap::Rect2D::new(100., 100., 352., 176.),
            hidden: false,
        }];
        let cfg = snap::SnapConfig::default();

        // X 贴近左对齐，Y 贴近 16dp 黄金间距（100 + 176 + 16 = 292）
        let res = snap::snap_drag(
            "b",
            snap::Rect2D::new(104., 288., 352., 176.),
            screen,
            &peers,
            cfg,
        );
        assert!((res.rect.x - 100.0).abs() < 1e-6);
        assert!((res.rect.y - 292.0).abs() < 1e-6);
        assert_eq!(res.guides.len(), 2);
        assert_eq!(res.stack_target, None);

        // 重叠度 > 55% 时触发智能叠放预览目标，不产生冲突参考线
        let overlap = snap::snap_drag(
            "b",
            snap::Rect2D::new(112., 112., 352., 176.),
            screen,
            &peers,
            cfg,
        );
        assert_eq!(overlap.stack_target.as_deref(), Some("a"));
        assert!(overlap.guides.is_empty());
    }

    #[test]
    fn snap_resize_snaps_right_and_bottom_to_peer() {
        let peers = vec![snap::SnapPeer {
            id: "a".into(),
            rect: snap::Rect2D::new(100., 100., 400., 220.),
            hidden: false,
        }];
        let (r, guides) = snap::snap_resize(
            "b",
            false,
            true,
            false,
            true,
            snap::Rect2D::new(100., 340., 395., 216.),
            &peers,
            snap::SnapConfig::default(),
        );
        assert!((r.right() - 500.0).abs() < 1e-6);
        assert!((r.bottom() - 560.0).abs() < 1e-6);
        assert_eq!(guides.len(), 2);
    }

    #[test]
    fn stack_manager_merge_step_unpack_and_context_cooldown_work() {
        let mut mgr = stack::StackManager::new();
        let (sid, updates) = mgr.merge_pair("w-music", "w-focus", "");
        assert!(!sid.is_empty());
        assert_eq!(updates.len(), 2);

        let mut p1 = sample_placement("w-music", Kind::Music, 100, 100);
        let mut p2 = sample_placement("w-focus", Kind::Focus, 100, 100);
        p1.stack_id = sid.clone();
        p1.stack_order = 0;
        p2.stack_id = sid.clone();
        p2.stack_order = 1;
        let items = vec![p1.clone(), p2.clone()];

        // 合并后当前在专注(idx=1)，手动翻页回到音乐(idx=0)
        assert!(mgr.step_by_member("w-focus", -1, 10.0));
        assert_eq!(mgr.status_of("w-music"), Some((0, 2, true)));
        assert_eq!(mgr.status_of("w-focus"), Some((0, 2, false)));

        // 手动后 5s（<15s冷却）内，专注运行时不应抢翻页面
        let sig = stack::ContextSignals {
            focus_running: true,
            ..Default::default()
        };
        assert!(!mgr.auto_rotate(15.0, sig, &items));
        assert_eq!(mgr.status_of("w-music"), Some((0, 2, true)));

        // 超过 15s 后自动切至专注
        assert!(mgr.auto_rotate(30.0, sig, &items));
        assert_eq!(mgr.status_of("w-focus"), Some((1, 2, true)));

        // 拆出成员后不足 2 项，自动解散
        let unpacked = mgr.unpack_member("w-focus");
        assert_eq!(unpacked.len(), 2);
        assert!(mgr.stacks.is_empty());
    }

    #[test]
    fn dotlayout_roundtrip_scales_across_resolutions_and_rejects_bad_input() {
        let mut p = sample_placement("clock-1", Kind::Clock, 960, 540);
        p.material = "acrylic".into();
        p.opacity = 0.85;
        let src_screen = snap::Rect2D::new(0., 0., 1920., 1080.);
        let pack = layout_pack::from_placements("办公桌搭", "DotIsle", src_screen, &[p], &[]);
        let token = layout_pack::export_dotlayout(&pack).unwrap();
        assert!(token.starts_with("DOTLAYOUT1:"));

        // 导入到 2560x1440 新屏幕，相对位置应等比拉伸至 1280x720
        let dst_screen = snap::Rect2D::new(0., 0., 2560., 1440.);
        let (restored_pack, placements, _) =
            layout_pack::import_dotlayout(&token, dst_screen).unwrap();
        assert_eq!(restored_pack.name, "办公桌搭");
        assert_eq!(placements.len(), 1);
        assert!((placements[0].x - 1280).abs() <= 1);
        assert!((placements[0].y - 720).abs() <= 1);
        assert_eq!(placements[0].material, "acrylic");

        // 拒绝篡改和越界 JSON
        assert!(layout_pack::import_dotlayout("DOTLAYOUT1:not_valid", dst_screen).is_err());
    }

    #[test]
    fn efficiency_note_checkbox_dropzone_and_dashboard_work() {
        let mut dash = efficiency::DashboardState::default();
        assert!(dash.toggle());
        assert!(dash.tick(0.05));
        assert!(dash.opacity > 0.0);

        let src = "- [ ] 写完 Allegro\n- [x] 跑通测试";
        let toggled = efficiency::toggle_note_checkbox(src, 0).unwrap();
        assert_eq!(toggled, "- [x] 写完 Allegro\n- [x] 跑通测试");
        assert_eq!(efficiency::toggle_note_checkbox(src, 9), None);

        let mut dz = efficiency::DropzoneStore::default();
        assert!(dz.push_path("w1", std::path::Path::new("C:\\demo\\report.pdf")).is_some());
        assert!(dz.push_path("w1", std::path::Path::new("C:\\demo\\report.pdf")).is_none());
        assert_eq!(dz.items("w1")[0].tag, "DOC");
        dz.clear("w1");
        assert!(dz.items("w1").is_empty());
    }

    #[test]
    fn single_canvas_grouping_hit_test_and_input_region_work() {
        let mon = canvas::MonitorDesc {
            key: "M0".into(),
            phys_x: 0,
            phys_y: 0,
            phys_w: 1920,
            phys_h: 1080,
            scale: 1.0,
        };
        let p1 = sample_placement("c1", Kind::Clock, 100, 100);
        let p2 = sample_placement("c2", Kind::Todos, 120, 120);
        let stacks = stack::StackManager::new();
        let slots = canvas::plan_monitor_slots(&mon, &[mon.clone()], &[p1, p2], &stacks);
        assert_eq!(slots.len(), 2);
        // 重叠区优先命中 Z-order 更靠上的第二个卡片
        assert_eq!(canvas::hit_test(&slots, (140., 140.)), Some(1));
        assert_eq!(canvas::hit_test(&slots, (10., 10.)), None);

        // 非仪表盘状态只包含卡片区域；仪表盘开启时覆盖全屏（Medium = 368×176，1.5x => 552×264）
        let regions = canvas::union_input_rects(&slots, false, 1920, 1080, 1.5);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0], (150, 150, 702, 414, 36));
        let full = canvas::union_input_rects(&slots, true, 1920, 1080, 1.5);
        assert_eq!(full, vec![(0, 0, 1920, 1080, 0)]);
    }

    #[test]
    fn edit_grid_snap_and_auto_pack_produce_non_overlapping_slots() {
        let grid = snap::compute_grid(1920.0, 1080.0);
        assert!(grid.cols >= 4 && grid.rows >= 3);
        let raw = snap::Rect2D::new(grid.origin_x + 15.0, grid.origin_y + 12.0, 176.0, 176.0);
        let snapped = snap::snap_rect_to_grid(raw, &grid);
        assert!((snapped.x - grid.origin_x).abs() < 0.01);
        assert!((snapped.y - grid.origin_y).abs() < 0.01);

        // 两张重叠的卡片应能自动排到互不重叠的网格中
        let items = vec![
            ("a".into(), snap::Rect2D::new(grid.origin_x, grid.origin_y, 176.0, 176.0)),
            ("b".into(), snap::Rect2D::new(grid.origin_x + 10.0, grid.origin_y + 10.0, 352.0, 176.0)),
            ("c".into(), snap::Rect2D::new(grid.origin_x + 10.0, grid.origin_y + 10.0, 176.0, 176.0)),
        ];
        let packed = snap::auto_pack_grid(&items, &grid);
        assert_eq!(packed.len(), 3);
        for i in 0..packed.len() {
            for j in (i + 1)..packed.len() {
                assert_eq!(packed[i].1.intersection_area(packed[j].1), 0.0);
            }
        }
    }

    #[test]
    fn stage_spotlight_math_slash_clipboard_and_spaces_work() {
        assert_eq!(stage::eval_quick_math("128*36"), Some(4608.0));
        assert_eq!(stage::eval_quick_math("=(100 + 25) / 5"), Some(25.0));
        assert_eq!(stage::eval_quick_math("42"), None);
        assert_eq!(stage::format_math_value(25.0), "25");

        let mut idx = stage::AppIndex::default();
        idx.ensure_loaded();
        let math_res = stage::resolve_stage_results("12+30", &idx);
        assert!(matches!(math_res.first().map(|r| &r.kind), Some(stage::StageResultKind::MathCopy(s)) if s == "42"));

        let slash_res = stage::resolve_stage_results("/focus 45", &idx);
        assert!(matches!(
            slash_res.first().map(|r| &r.kind),
            Some(stage::StageResultKind::RunCommand(stage::StageSlashCmd::StartFocus(45)))
        ));

        let app_res = stage::resolve_stage_results("终端", &idx);
        assert!(!app_res.is_empty());

        let mut cb = stage::ClipboardHistory::default();
        for i in 0..12 {
            cb.push_text(&format!("第 {i} 条片段"));
        }
        assert_eq!(cb.items.len(), 8);
        assert_eq!(cb.items[0].text, "第 11 条片段");
        cb.push_text("第 10 条片段");
        assert_eq!(cb.items[0].text, "第 10 条片段");

        let mut wall = wallpaper::WallpaperCache::new();
        let mut buf = vec![0u8; 64 * 64 * 4];
        for px in buf.chunks_exact_mut(4) {
            px.copy_from_slice(&[180, 195, 220, 255]);
        }
        wall.set_synthetic_wallpaper(64, 64, buf);
        let frost = wall
            .crop_sidebar_frost("m1", (1920., 1080.), snap::Rect2D::new(1500., 12., 400., 1056.))
            .expect("sidebar silk frost");
        assert!(frost.image.width >= 48 && frost.image.height >= 96);
    }

    #[test]
    fn sidebar_block_layout_enforces_apple_ratios_and_packs_cleanly() {
        // 验证 Apple 标准区块尺寸比例：大=2小+间距，特大=2×2（4小）
        assert_eq!(Size::Small.dimensions(), (176.0, 176.0));
        assert_eq!(Size::Medium.dimensions(), (368.0, 176.0));
        assert_eq!(Size::Large.dimensions(), (368.0, 368.0));

        let widgets = stage::default_sidebar_widgets();
        let (slots, total_h) = stage::pack_sidebar_blocks(&widgets, 20.0, 100.0, 176.0, 16.0);
        assert_eq!(slots.len(), widgets.len());
        assert!(total_h > 368.0);

        // 校验每张卡片尺寸严格符合 1×1 / 2×1 / 2×2，且所有区块互不重叠
        for (slot, p) in slots.iter().zip(widgets.iter()) {
            let (ew, eh) = p.size.dimensions();
            assert!((slot.rect.w - ew).abs() < 0.01);
            assert!((slot.rect.h - eh).abs() < 0.01);
            assert!(slot.col + slot.span_x <= stage::SIDEBAR_COLS);
        }
        for i in 0..slots.len() {
            for j in (i + 1)..slots.len() {
                assert_eq!(slots[i].rect.intersection_area(slots[j].rect), 0.0);
            }
        }

        // 校验 1×1 + 2×1 + 1×1 会自动填补第一行第 2 列的空位，无空洞浪费
        let mut st = stage::StageState::default();
        st.sidebar_loaded = true;
        st.add_sidebar_widget(Kind::Clock, Size::Small, "");
        st.add_sidebar_widget(Kind::Weather, Size::Medium, "");
        st.add_sidebar_widget(Kind::Calendar, Size::Small, "");
        let (packed, _) = stage::pack_sidebar_blocks(&st.sidebar_widgets, 0.0, 0.0, 176.0, 16.0);
        assert_eq!((packed[0].col, packed[0].row), (0, 0));
        assert_eq!((packed[1].col, packed[1].row), (0, 1));
        assert_eq!((packed[2].col, packed[2].row), (1, 0));
    }
}
