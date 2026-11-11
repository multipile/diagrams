//! What each type of diagram promises of its picture, checked on its cases:
//! where the layout puts each element, read from the scene it makes, and
//! what the page and the picture show of it.

mod common;

use common::*;
use diagram_document::geometry::crossings;
use diagram_document::scene::*;
use diagram_pipeline::{Diagram, StillFormat, format::Body};

fn laid_out(case: &Case) -> Scene {
    diagram_document::layout(&case.diagram())
}

fn rect(scene: &Scene, id: &str) -> Rect {
    scene
        .element(id)
        .unwrap_or_else(|| panic!("no element {id}"))
        .shape
        .bounds()
}

fn boxed<'s>(scene: &'s Scene, id: &str) -> &'s BoxShape {
    match &scene.element(id).unwrap_or_else(|| panic!("no element {id}")).shape {
        Shape::Box(shape) => shape,
        other => panic!("{id} is not a box: {other:?}"),
    }
}

fn lines(scene: &Scene) -> Vec<(&str, &LineShape)> {
    scene
        .elements
        .iter()
        .filter_map(|element| match &element.shape {
            Shape::Line(line) => Some((element.id.as_str(), line)),
            _ => None,
        })
        .collect()
}

fn boxes(scene: &Scene) -> Vec<(&str, Rect)> {
    scene
        .elements
        .iter()
        .filter_map(|element| match &element.shape {
            Shape::Box(shape) => Some((element.id.as_str(), shape.rect)),
            _ => None,
        })
        .collect()
}

fn regions(scene: &Scene, kind: RegionKind) -> Vec<(&str, &RegionShape)> {
    scene
        .elements
        .iter()
        .filter_map(|element| match &element.shape {
            Shape::Region(region) if region.kind == kind => Some((element.id.as_str(), region)),
            _ => None,
        })
        .collect()
}

fn inside(inner: Rect, outer: Rect) -> bool {
    inner.x >= outer.x && inner.y >= outer.y && inner.right() <= outer.right() && inner.bottom() <= outer.bottom()
}

/// No line of a scene passes behind a box that is not one of its ends.
fn no_line_behind_a_box(case: &Case, scene: &Scene, ends: impl Fn(&str) -> Vec<String>) {
    let all = boxes(scene);
    for (id, line) in lines(scene) {
        let own = ends(id);
        let others: Vec<Rect> = all
            .iter()
            .filter(|(box_id, _)| !own.iter().any(|end| box_id.ends_with(&format!(":{end}"))))
            .map(|(_, rect)| rect.grow(-1))
            .collect();
        assert_eq!(
            crossings(&line.points, &others),
            0,
            "{}: {id} passes behind a box",
            case.name
        );
    }
}

/// The two keys a line's id names: `line:a->b`.
fn line_ends(id: &str) -> Vec<String> {
    let rest = id.split_once(':').map_or(id, |(_, rest)| rest);
    let rest = rest.split('#').next().unwrap();
    rest.split("->")
        .map(|key| key.split(':').next().unwrap().to_string())
        .collect()
}

fn diagram_of<T>(case: &Case, pick: impl Fn(Body) -> Option<T>) -> T {
    let Diagram { body, .. } = case.diagram();
    pick(body).unwrap_or_else(|| panic!("{} is of another type", case.name))
}

mod architecture {
    use super::*;

    #[test]
    fn tiers_run_along_the_flow() {
        for case in cases("architecture") {
            let body = diagram_of(&case, |b| if let Body::Architecture(a) = b { Some(a) } else { None });
            let scene = laid_out(&case);
            let right = body.flow == diagram_pipeline::format::Flow::Right;
            for pair in body.tiers.windows(2) {
                let a = rect(&scene, &format!("box:{}", pair[0].keys[0]));
                let b = rect(&scene, &format!("box:{}", pair[1].keys[0]));
                if right {
                    assert!(a.right() < b.x, "{}: a tier is not right of the one before", case.name);
                } else {
                    assert!(a.bottom() < b.y, "{}: a tier is not below the one before", case.name);
                }
            }
        }
    }

    #[test]
    fn a_held_box_is_drawn_inside_its_holder() {
        let case = &cases("architecture")[0];
        let body = diagram_of(case, |b| if let Body::Architecture(a) = b { Some(a) } else { None });
        let scene = laid_out(case);
        let holder = body
            .boxes
            .iter()
            .find(|held| !held.holds.is_empty())
            .expect("a box that holds boxes");
        let outer = rect(&scene, &format!("box:{}", holder.item.key));
        assert!(
            boxed(&scene, &format!("box:{}", holder.item.key)).top,
            "a holder's name is at its top"
        );
        for inner in &holder.holds {
            assert!(
                inside(rect(&scene, &format!("box:{}", inner.key)), outer),
                "{} is outside its holder",
                inner.key
            );
        }
    }

    #[test]
    fn a_zone_surrounds_its_tier_and_a_trust_boundary_is_dashed() {
        for case in cases("architecture") {
            let body = diagram_of(&case, |b| if let Body::Architecture(a) = b { Some(a) } else { None });
            let scene = laid_out(&case);
            let zones = regions(&scene, RegionKind::Zone);
            for (index, tier) in body.tiers.iter().enumerate() {
                let Some(zone) = &tier.zone else { continue };
                let (_, region) = zones
                    .iter()
                    .find(|(id, _)| *id == format!("zone:{index}"))
                    .unwrap_or_else(|| panic!("{}: no zone drawn for tier {index}", case.name));
                assert_eq!(region.label.as_deref(), Some(zone.label.as_str()));
                assert_eq!(region.dashed, zone.trust, "{}: {}", case.name, zone.label);
                for key in &tier.keys {
                    assert!(
                        inside(rect(&scene, &format!("box:{key}")), region.rect),
                        "{}: {key} is outside its zone",
                        case.name
                    );
                }
            }
        }
    }

    #[test]
    fn a_line_shows_its_label_and_its_kind() {
        let case = cases("architecture")
            .into_iter()
            .find(|c| c.name.ends_with("a-web-app-and-its-api"))
            .unwrap();
        let scene = laid_out(&case);
        let (_, http) = lines(&scene)
            .into_iter()
            .find(|(id, _)| *id == "line:cli->api")
            .unwrap();
        assert_eq!(http.label.as_ref().map(|l| l.text.as_str()), Some("HTTP"));
        assert_eq!(http.tone, Tone::Link, "a link is drawn in the link colour");
        let (_, write) = lines(&scene)
            .into_iter()
            .find(|(id, _)| *id == "line:api->blobs")
            .unwrap();
        assert!(write.dashed, "an async line is dashed");
        assert_eq!(write.head, Head::Open, "an async line has an open head");
    }

    #[test]
    fn no_line_passes_behind_a_box() {
        for case in cases("architecture") {
            let body = diagram_of(&case, |b| if let Body::Architecture(a) = b { Some(a) } else { None });
            let scene = laid_out(&case);
            let holders: Vec<(String, Vec<String>)> = body
                .boxes
                .iter()
                .map(|held| {
                    (
                        held.item.key.clone(),
                        held.holds.iter().map(|i| i.key.clone()).collect(),
                    )
                })
                .collect();
            no_line_behind_a_box(&case, &scene, |id| {
                let mut ends = line_ends(id);
                // A line to a held box crosses its holder's edge to reach it.
                for (holder, held) in &holders {
                    if ends.iter().any(|end| held.contains(end)) {
                        ends.push(holder.clone());
                    }
                }
                ends
            });
        }
    }
}

mod sequence {
    use super::*;

    fn messages(scene: &Scene) -> Vec<(&str, &LineShape)> {
        lines(scene)
            .into_iter()
            .filter(|(id, _)| id.starts_with("message:"))
            .collect()
    }

    #[test]
    fn actors_stand_in_a_row_in_the_order_written() {
        for case in cases("sequence") {
            let body = diagram_of(&case, |b| if let Body::Sequence(s) = b { Some(s) } else { None });
            let scene = laid_out(&case);
            let rects: Vec<Rect> = body
                .actors
                .iter()
                .map(|a| rect(&scene, &format!("actor:{}", a.key)))
                .collect();
            for pair in rects.windows(2) {
                assert_eq!(pair[0].y, pair[1].y, "{}: actors stand in one row", case.name);
                assert!(
                    pair[0].right() < pair[1].x,
                    "{}: actors stand in the order written",
                    case.name
                );
            }
        }
    }

    #[test]
    fn time_runs_down_one_message_after_another() {
        for case in cases("sequence") {
            let scene = laid_out(&case);
            let ys: Vec<i32> = messages(&scene).iter().map(|(_, line)| line.points[0].y).collect();
            assert!(ys.len() >= 5, "{}", case.name);
            assert!(
                ys.windows(2).all(|pair| pair[0] < pair[1]),
                "{}: a message points up: {ys:?}",
                case.name
            );
            // And each is revealed after the one before.
            let steps: Vec<u32> = scene
                .elements
                .iter()
                .filter(|e| e.id.starts_with("message:"))
                .map(|e| e.step)
                .collect();
            assert!(steps.windows(2).all(|pair| pair[0] < pair[1]), "{}", case.name);
        }
    }

    #[test]
    fn a_call_answered_by_a_reply_holds_control_until_then() {
        for case in cases("sequence") {
            let scene = laid_out(&case);
            let bars = regions(&scene, RegionKind::Activation);
            assert!(!bars.is_empty(), "{}: no actor holds control", case.name);
            let lifelines = regions(&scene, RegionKind::Lifeline);
            for (id, bar) in bars {
                let x = bar.rect.cx();
                assert!(
                    lifelines.iter().any(|(_, line)| line.rect.x == x),
                    "{}: {id} is on no lifeline",
                    case.name
                );
            }
        }
    }

    #[test]
    fn a_fragment_frames_its_messages_with_its_guard() {
        for case in cases("sequence") {
            let scene = laid_out(&case);
            let fragments = regions(&scene, RegionKind::Fragment);
            assert_eq!(fragments.len(), 1, "{}: one opt, loop or alt", case.name);
            let (_, frame) = fragments[0];
            let label = frame.label.clone().unwrap();
            assert!(
                ["OPT [", "LOOP [", "ALT ["].iter().any(|kind| label.starts_with(kind)),
                "{label}"
            );
            let framed = messages(&scene)
                .iter()
                .filter(|(_, line)| frame.rect.y < line.points[0].y && line.points[0].y < frame.rect.bottom())
                .count();
            assert!(framed >= 1, "{}: the frame holds no message", case.name);
        }
    }

    #[test]
    fn a_reply_is_dashed_and_a_notification_has_an_open_head() {
        let case = cases("sequence")
            .into_iter()
            .find(|c| c.name.ends_with("a-pull-request-is-reviewed"))
            .unwrap();
        let scene = laid_out(&case);
        let all = messages(&scene);
        let (_, push) = all
            .iter()
            .find(|(id, _)| id.starts_with("message:github->action:push"))
            .unwrap();
        assert!(push.dashed);
        assert_eq!(push.head, Head::Open);
        let (_, markdown) = all
            .iter()
            .find(|(id, _)| id.starts_with("message:pile->action:markdown"))
            .unwrap();
        assert!(markdown.dashed);
        assert_eq!(markdown.head, Head::Arrow);
        let (_, comment) = all
            .iter()
            .find(|(id, _)| id.starts_with("message:action->github:comment"))
            .unwrap();
        assert_eq!(comment.tone, Tone::Link, "a call with link=#true is in the link colour");
    }
}

mod state {
    use super::*;

    #[test]
    fn states_follow_the_order_the_start_reaches_them_in() {
        let case = cases("state")
            .into_iter()
            .find(|c| c.name.ends_with("a-diagram-file"))
            .unwrap();
        let scene = laid_out(&case);
        let x = |key: &str| rect(&scene, &format!("state:{key}")).x;
        assert!(x("written") < x("read"));
        assert!(x("read") < x("refused"));
        assert!(x("refused") < x("rendered"));
        assert!(x("rendered") < x("stale"));
    }

    #[test]
    fn the_start_has_a_filled_dot_and_each_end_a_ringed_one() {
        for case in cases("state") {
            let body = diagram_of(&case, |b| if let Body::State(s) = b { Some(s) } else { None });
            let scene = laid_out(&case);
            let dots: Vec<&DotShape> = scene
                .elements
                .iter()
                .filter_map(|e| if let Shape::Dot(d) = &e.shape { Some(d) } else { None })
                .collect();
            assert_eq!(dots.iter().filter(|d| !d.ring).count(), 1, "{}: one start", case.name);
            let ends = body.states.iter().filter(|s| s.end).count();
            assert_eq!(
                dots.iter().filter(|d| d.ring).count(),
                ends,
                "{}: a ring for each end",
                case.name
            );
        }
    }

    #[test]
    fn a_transition_says_its_event_guard_and_action() {
        let case = cases("state")
            .into_iter()
            .find(|c| c.name.ends_with("a-pull-request"))
            .unwrap();
        let scene = laid_out(&case);
        let labels: Vec<String> = lines(&scene)
            .iter()
            .filter_map(|(_, l)| l.label.as_ref().map(|l| l.text.clone()))
            .collect();
        assert!(labels.contains(&"PUSH / DISMISS".to_string()), "{labels:?}");
        let case = cases("state")
            .into_iter()
            .find(|c| c.name.ends_with("an-issue"))
            .unwrap();
        let scene = laid_out(&case);
        let labels: Vec<String> = lines(&scene)
            .iter()
            .filter_map(|(_, l)| l.label.as_ref().map(|l| l.text.clone()))
            .collect();
        assert!(labels.contains(&"DROP [NOT PLANNED]".to_string()), "{labels:?}");
    }

    #[test]
    fn a_transition_from_any_state_is_one_note() {
        let case = cases("state")
            .into_iter()
            .find(|c| c.name.ends_with("a-diagram-file"))
            .unwrap();
        let scene = laid_out(&case);
        let any = scene.element("any").expect("a note for the transition from any state");
        assert!(
            matches!(&any.shape, Shape::Text(text) if text.text.contains("DELETED") && text.text.contains("GIT RM"))
        );
        assert!(
            lines(&scene).iter().all(|(id, _)| !id.contains("->deleted")),
            "no line from each state"
        );
    }

    #[test]
    fn a_state_that_returns_to_itself_loops_over_it() {
        let case = cases("state")
            .into_iter()
            .find(|c| c.name.ends_with("an-issue"))
            .unwrap();
        let scene = laid_out(&case);
        let state = rect(&scene, "state:progress");
        let (_, own) = lines(&scene)
            .into_iter()
            .find(|(id, _)| id.starts_with("transition:progress->progress"))
            .unwrap();
        assert!(own.points.iter().all(|p| p.y <= state.y), "the loop is over the state");
        assert!(own.points.iter().all(|p| state.x <= p.x && p.x <= state.right()));
    }
}

mod swimlane {
    use super::*;

    #[test]
    fn a_step_sits_in_its_lanes_band() {
        for case in cases("swimlane") {
            let body = diagram_of(&case, |b| if let Body::Swimlane(s) = b { Some(s) } else { None });
            let scene = laid_out(&case);
            for lane in &body.lanes {
                let band = rect(&scene, &format!("lane:{}", lane.key));
                for step in &lane.steps {
                    assert!(
                        inside(rect(&scene, &format!("step:{}", step.key)), band),
                        "{}: {} is out of its lane",
                        case.name,
                        step.key
                    );
                }
            }
        }
    }

    #[test]
    fn a_step_is_right_of_the_steps_that_lead_to_it() {
        for case in cases("swimlane") {
            let body = diagram_of(&case, |b| if let Body::Swimlane(s) = b { Some(s) } else { None });
            let scene = laid_out(&case);
            let forward: Vec<_> = body
                .lines
                .iter()
                .filter(|line| !(case.name.ends_with("a-review-asks-for-changes") && line.from == "fix"))
                .collect();
            for line in forward {
                let (from, to) = (
                    rect(&scene, &format!("step:{}", line.from)),
                    rect(&scene, &format!("step:{}", line.to)),
                );
                assert!(
                    from.x < to.x,
                    "{}: {} is not right of {}",
                    case.name,
                    line.to,
                    line.from
                );
            }
        }
    }

    #[test]
    fn a_line_back_runs_over_the_lane_and_pushes_nothing_on() {
        let case = cases("swimlane")
            .into_iter()
            .find(|c| c.name.ends_with("a-review-asks-for-changes"))
            .unwrap();
        let scene = laid_out(&case);
        assert!(rect(&scene, "step:read").x < rect(&scene, "step:fix").x);
        let (_, back) = lines(&scene)
            .into_iter()
            .find(|(id, _)| *id == "line:fix->read")
            .unwrap();
        let top = rect(&scene, "step:fix").y.min(rect(&scene, "step:read").y);
        assert!(
            back.points.iter().any(|p| p.y < top),
            "the line back runs over the steps"
        );
    }

    #[test]
    fn a_handoff_crosses_from_one_lane_to_the_next() {
        let case = cases("swimlane")
            .into_iter()
            .find(|c| c.name.ends_with("the-cli-is-released"))
            .unwrap();
        let scene = laid_out(&case);
        let (_, handoff) = lines(&scene)
            .into_iter()
            .find(|(id, _)| *id == "line:tag->build")
            .unwrap();
        let (author, ci) = (rect(&scene, "lane:maintainer"), rect(&scene, "lane:cli"));
        let (first, last) = (handoff.points[0], handoff.points[handoff.points.len() - 1]);
        assert!(author.y <= first.y && first.y <= author.bottom());
        assert!(ci.y <= last.y && last.y <= ci.bottom());
        no_line_behind_a_box(&case, &scene, line_ends);
    }
}

mod process {
    use super::*;

    #[test]
    fn a_task_sits_where_its_lane_meets_its_column() {
        for case in cases("process") {
            let body = diagram_of(&case, |b| if let Body::Process(p) = b { Some(p) } else { None });
            let scene = laid_out(&case);
            for lane in &body.lanes {
                let band = rect(&scene, &format!("lane:{}", lane.key));
                for task in &lane.tasks {
                    let at = rect(&scene, &format!("task:{}", task.item.key));
                    let column = rect(&scene, &format!("column:{}", task.column));
                    assert!(inside(at, band), "{}: {} is out of its lane", case.name, task.item.key);
                    assert!(
                        inside(at, column),
                        "{}: {} is out of its column",
                        case.name,
                        task.item.key
                    );
                }
            }
        }
    }

    #[test]
    fn a_task_shows_its_lanes_badge_its_tool_and_its_data() {
        let case = cases("process")
            .into_iter()
            .find(|c| c.name.ends_with("a-diagram-is-added"))
            .unwrap();
        let scene = laid_out(&case);
        let png = boxed(&scene, "task:png");
        assert_eq!(png.corner.as_deref(), Some("PIP"));
        assert_eq!(png.foot.as_deref(), Some("diagram render"));
        assert_eq!(png.tags, vec!["LS→FL".to_string()]);
    }

    #[test]
    fn columns_are_numbered_in_order_and_one_is_focal() {
        for case in cases("process") {
            let scene = laid_out(&case);
            let columns = regions(&scene, RegionKind::Column);
            for (n, (_, column)) in columns.iter().enumerate() {
                assert!(
                    column.label.as_ref().unwrap().starts_with(&format!("{:02} ", n + 1)),
                    "{}",
                    case.name
                );
            }
            assert!(columns.iter().filter(|(_, c)| c.focal).count() <= 1, "{}", case.name);
            assert!(
                columns.windows(2).all(|pair| pair[0].1.rect.x < pair[1].1.rect.x),
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn the_focal_tasks_lines_are_drawn_in_the_accent_and_a_trigger_is_dashed() {
        let case = cases("process")
            .into_iter()
            .find(|c| c.name.ends_with("a-diagram-is-added"))
            .unwrap();
        let scene = laid_out(&case);
        let all = lines(&scene);
        let tone = |id: &str| all.iter().find(|(line, _)| *line == id).unwrap().1.tone;
        assert_eq!(tone("line:parse->png"), Tone::Accent);
        assert_eq!(tone("line:png->again"), Tone::Accent);
        assert_ne!(tone("line:file->parse"), Tone::Accent);
        assert!(
            all.iter().find(|(id, _)| *id == "line:again->merge").unwrap().1.dashed,
            "a trigger is dashed"
        );
    }
}

mod tree {
    use super::*;

    #[test]
    fn a_parent_is_centred_over_its_children() {
        for case in cases("tree") {
            let body = diagram_of(&case, |b| if let Body::Tree(t) = b { Some(t) } else { None });
            let scene = laid_out(&case);
            let down = body.flow == diagram_pipeline::format::Flow::Down;
            for branch in body.root.walk() {
                if branch.children.is_empty() {
                    continue;
                }
                let parent = rect(&scene, &format!("item:{}", branch.item.key));
                let first = rect(&scene, &format!("item:{}", branch.children[0].item.key));
                let last = rect(&scene, &format!("item:{}", branch.children.last().unwrap().item.key));
                if down {
                    let middle = (first.cx() + last.cx()) / 2;
                    assert!(
                        (parent.cx() - middle).abs() <= 4,
                        "{}: {} is not over its children",
                        case.name,
                        branch.item.key
                    );
                    assert!(parent.bottom() < first.y, "{}", case.name);
                } else {
                    let middle = (first.cy() + last.cy()) / 2;
                    assert!(
                        (parent.cy() - middle).abs() <= 4,
                        "{}: {} is not beside its children",
                        case.name,
                        branch.item.key
                    );
                    assert!(parent.right() < first.x, "{}", case.name);
                }
            }
        }
    }

    #[test]
    fn siblings_do_not_overlap() {
        for case in cases("tree") {
            let scene = laid_out(&case);
            let all = boxes(&scene);
            for (i, (a, ra)) in all.iter().enumerate() {
                for (b, rb) in &all[i + 1..] {
                    let apart = ra.right() <= rb.x || rb.right() <= ra.x || ra.bottom() <= rb.y || rb.bottom() <= ra.y;
                    assert!(apart, "{}: {a} and {b} overlap", case.name);
                }
            }
        }
    }

    #[test]
    fn lines_run_as_a_bus_with_no_heads() {
        for case in cases("tree") {
            let scene = laid_out(&case);
            for (id, line) in lines(&scene) {
                assert_eq!(line.head, Head::None, "{}: {id}", case.name);
                for segment in line.points.windows(2) {
                    assert!(
                        segment[0].x == segment[1].x || segment[0].y == segment[1].y,
                        "{}: {id} is not at right angles",
                        case.name
                    );
                }
            }
        }
    }
}

mod dependency {
    use super::*;

    #[test]
    fn a_dependency_sits_a_rank_further_along() {
        for case in cases("dependency") {
            let body = diagram_of(&case, |b| if let Body::Dependency(d) = b { Some(d) } else { None });
            let scene = laid_out(&case);
            let down = body.flow == diagram_pipeline::format::Flow::Down;
            for line in &body.lines {
                let (from, to) = (
                    rect(&scene, &format!("box:{}", line.from)),
                    rect(&scene, &format!("box:{}", line.to)),
                );
                if down {
                    assert!(
                        from.bottom() < to.y,
                        "{}: {} is not below {}",
                        case.name,
                        line.to,
                        line.from
                    );
                } else {
                    assert!(
                        from.right() < to.x,
                        "{}: {} is not right of {}",
                        case.name,
                        line.to,
                        line.from
                    );
                }
            }
        }
    }

    #[test]
    fn a_box_several_depend_on_says_how_many() {
        let case = cases("dependency")
            .into_iter()
            .find(|c| c.name.ends_with("the-cli-crates"))
            .unwrap();
        let scene = laid_out(&case);
        assert_eq!(boxed(&scene, "box:storage").tags, vec!["3 IN".to_string()]);
        assert_eq!(boxed(&scene, "box:piles").tags, vec!["2 IN".to_string()]);
        assert!(boxed(&scene, "box:graph").tags.is_empty(), "one dependent says nothing");
    }

    #[test]
    fn a_box_that_depends_on_nothing_is_a_store() {
        let case = cases("dependency")
            .into_iter()
            .find(|c| c.name.ends_with("the-cli-crates"))
            .unwrap();
        let scene = laid_out(&case);
        assert_eq!(boxed(&scene, "box:storage").look, diagram_pipeline::format::Look::Store);
        assert_eq!(boxed(&scene, "box:guide").look, diagram_pipeline::format::Look::Store);
        assert_eq!(boxed(&scene, "box:pile").look, diagram_pipeline::format::Look::Plain);
        let case = cases("dependency")
            .into_iter()
            .find(|c| c.name.ends_with("the-diagram-crates"))
            .unwrap();
        let scene = laid_out(&case);
        assert_eq!(
            boxed(&scene, "box:takumi").look,
            diagram_pipeline::format::Look::External,
            "an explicit look stays"
        );
    }

    #[test]
    fn the_cycle_runs_outside_in_the_accent() {
        let case = cases("dependency")
            .into_iter()
            .find(|c| c.name.ends_with("a-cycle-between-two-crates"))
            .unwrap();
        let scene = laid_out(&case);
        let (_, cycle) = lines(&scene)
            .into_iter()
            .find(|(id, _)| id.starts_with("cycle:"))
            .unwrap();
        assert_eq!(cycle.tone, Tone::Accent);
        assert_eq!(cycle.label.as_ref().unwrap().text, "CYCLE");
        let rightmost = boxes(&scene).iter().map(|(_, r)| r.right()).max().unwrap();
        assert!(
            cycle.points.iter().any(|p| p.x > rightmost),
            "the cycle runs around the outside"
        );
    }

    #[test]
    fn no_line_passes_behind_a_box() {
        for case in cases("dependency") {
            let scene = laid_out(&case);
            no_line_behind_a_box(&case, &scene, line_ends);
        }
    }
}

mod nested {
    use super::*;

    #[test]
    fn each_level_is_inside_the_one_before_by_the_same_space() {
        for case in cases("nested") {
            let scene = laid_out(&case);
            let levels: Vec<Rect> = regions(&scene, RegionKind::Level).iter().map(|(_, r)| r.rect).collect();
            assert!(levels.len() >= 3, "{}", case.name);
            let gaps: Vec<(i32, i32, i32)> = levels
                .windows(2)
                .map(|pair| {
                    (
                        pair[1].x - pair[0].x,
                        pair[0].right() - pair[1].right(),
                        pair[0].bottom() - pair[1].bottom(),
                    )
                })
                .collect();
            for pair in levels.windows(2) {
                assert!(
                    inside(pair[1], pair[0]),
                    "{}: a level is not inside the one before",
                    case.name
                );
            }
            assert!(
                gaps.windows(2).all(|pair| pair[0] == pair[1]),
                "{}: the space between levels differs: {gaps:?}",
                case.name
            );
        }
    }

    #[test]
    fn a_level_that_is_a_pile_is_named_as_one() {
        let case = cases("nested")
            .into_iter()
            .find(|c| c.name.ends_with("which-pile-a-path-is-in"))
            .unwrap();
        let scene = laid_out(&case);
        let labels: Vec<String> = regions(&scene, RegionKind::Level)
            .iter()
            .map(|(_, r)| r.label.clone().unwrap())
            .collect();
        assert_eq!(labels[1], "@diagrams");
        assert_eq!(labels[2], "@html");
    }

    #[test]
    fn a_label_fits_inside_its_level() {
        let renderer = renderer();
        for case in cases("nested") {
            let scene = laid_out(&case);
            for (_, level) in regions(&scene, RegionKind::Level) {
                let text = level.label.clone().unwrap().chars().count()
                    + level.sub.as_deref().map_or(0, |s| s.chars().count() + 2);
                assert!(
                    (text as i32 * 6) + 20 <= level.rect.w,
                    "{}: {:?} does not fit",
                    case.name,
                    level.label
                );
            }
            // And the picture is drawn: the innermost level carries the accent.
            let png = diagram_pipeline::still(&renderer, &case.source, StillFormat::Png).unwrap();
            assert!(Picture::png(&png).accented() > 50, "{}", case.name);
        }
    }
}

mod treemap {
    use super::*;

    #[test]
    fn each_part_is_its_share_of_the_area() {
        for case in cases("treemap") {
            let body = diagram_of(&case, |b| if let Body::Treemap(t) = b { Some(t) } else { None });
            let scene = laid_out(&case);
            let total: f64 = body.parts.iter().map(|p| p.value).sum();
            let cells: Vec<(f64, Rect)> = body
                .parts
                .iter()
                .map(|p| (p.value / total, rect(&scene, &format!("part:{}", p.item.key))))
                .collect();
            // The gap between cells is taken from each, so shares are
            // compared on the cells grown back by it.
            let area: f64 = cells.iter().map(|(_, r)| ((r.w + 4) * (r.h + 4)) as f64).sum();
            for (share, r) in &cells {
                let drawn = ((r.w + 4) * (r.h + 4)) as f64 / area;
                assert!(
                    (drawn - share).abs() < 0.02,
                    "{}: a part of {share:.3} is drawn as {drawn:.3}",
                    case.name
                );
            }
        }
    }

    #[test]
    fn larger_parts_are_filled_stronger() {
        for case in cases("treemap") {
            let body = diagram_of(&case, |b| if let Body::Treemap(t) = b { Some(t) } else { None });
            let scene = laid_out(&case);
            let mut parts: Vec<(f64, f32)> = body
                .parts
                .iter()
                .map(|p| (p.value, boxed(&scene, &format!("part:{}", p.item.key)).fill.unwrap()))
                .collect();
            parts.sort_by(|a, b| b.0.total_cmp(&a.0));
            assert!(
                parts.windows(2).all(|pair| pair[0].1 >= pair[1].1),
                "{}: {parts:?}",
                case.name
            );
        }
    }

    #[test]
    fn a_part_too_small_for_its_name_is_named_under_the_map() {
        let case = cases("treemap")
            .into_iter()
            .find(|c| c.name.ends_with("eight-parts"))
            .unwrap();
        let scene = laid_out(&case);
        let captions: Vec<String> = scene
            .elements
            .iter()
            .filter_map(|e| {
                if let Shape::Text(t) = &e.shape {
                    Some(t.text.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(captions.iter().any(|c| c.contains("TAP FORMULA")), "{captions:?}");
        assert!(captions.iter().any(|c| c.starts_with("AREA: FILES")), "{captions:?}");
        assert!(
            captions.iter().any(|c| c.starts_with("SOURCE: PILE LIST")),
            "{captions:?}"
        );
        assert_eq!(
            boxed(&scene, "part:pile").name,
            "pile",
            "a part with room shows its name"
        );
    }

    #[test]
    fn a_part_shows_its_value_and_its_share() {
        let case = cases("treemap")
            .into_iter()
            .find(|c| c.name.ends_with("the-code-by-crate"))
            .unwrap();
        let scene = laid_out(&case);
        assert_eq!(boxed(&scene, "part:html").foot.as_deref(), Some("2400 · 51%"));
    }
}
