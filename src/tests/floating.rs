use client::ClientId;
use insta::assert_snapshot;
use niri_config::Config;
use niri_ipc::SizeChange;
use smithay::utils::Point;
use wayland_client::protocol::wl_surface::WlSurface;

use super::*;

// Sets up a fixture with two outputs and 100×100 window.
fn set_up() -> (Fixture, ClientId, WlSurface) {
    set_up_with_config(Config::default())
}

fn set_up_with_config(config: Config) -> (Fixture, ClientId, WlSurface) {
    let mut f = Fixture::with_config(config);
    f.add_output(1, (1920, 1080));
    f.add_output(2, (1280, 720));

    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(100, 100);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    (f, id, surface)
}

#[test]
fn unfocus_preserves_current_size() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.roundtrip(id);

    // Change window size while it's floating.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Focus a different output which should drop the Activated state.
    f.niri_focus_output(2);

    f.double_roundtrip(id);

    // This should request 200 × 200 because that's the current window size.
    let window = f.client(id).window(&surface);
    assert_snapshot!(
        window.format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: []"
    );

    // Change window size again.
    let window = f.client(id).window(&surface);
    window.set_size(300, 300);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Focus the first output which should add back the Activated state.
    f.niri_focus_output(1);

    f.double_roundtrip(id);

    // This should request 300 × 300 because that's the current window size.
    let window = f.client(id).window(&surface);
    assert_snapshot!(
        window.format_recent_configures(),
        @"size: 300 × 300, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_to_different_size() {
    let (mut f, id, surface) = set_up();
    let _ = f.client(id).window(&surface).recent_configures();

    // Commit in response to the Activated state change configure.
    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);

    f.niri().layout.toggle_window_floating(None);
    f.niri().layout.set_column_width(SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    // This should request the new size, 500 × 100.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    // Focus a different output which should drop the Activated state.
    f.niri_focus_output(2);
    f.double_roundtrip(id);
    // This should request the new size since the window hasn't committed yet.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: []"
    );

    // Ack but don't commit yet.
    let window = f.client(id).window(&surface);
    window.ack_last();
    f.roundtrip(id);
    // Add the activated state.
    f.niri_focus_output(1);
    f.double_roundtrip(id);
    // This should request the new size since the window hasn't committed yet.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    // Commit but with some different size.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.commit();
    f.double_roundtrip(id);
    // This shouldn't request anything.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    // Drop the Activated state.
    f.niri_focus_output(2);
    f.double_roundtrip(id);
    // This should request the current window size rather than keep requesting 500 × 100.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: []"
    );
}

#[test]
fn set_window_width_uses_current_height() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    // Resize to something different on both axes.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Request a width change.
    f.niri().layout.set_column_width(SizeChange::SetFixed(500));

    f.double_roundtrip(id);

    // This should use the current window height (200), rather than the initial window height (100).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn set_window_height_uses_current_width() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    // Resize to something different on both axes.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Request a width change.
    f.niri()
        .layout
        .set_window_height(None, SizeChange::SetFixed(500));

    f.double_roundtrip(id);

    // This should use the current window width (200), rather than the initial window width (100).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 500, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_to_same_size() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    // Resize to something different.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Request a size change to the same size.
    f.niri().layout.set_column_width(SizeChange::SetFixed(200));

    f.double_roundtrip(id);

    // This needn't request anything because we're already that size; the size in the current
    // server state matches the requested size.
    //
    // FIXME: However, currently it will request the size anyway because the code checks the
    // current server state, and the last size niri requested of the window was 100×100 (even if
    // the window already acked and committed in response).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_to_different_then_same() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    // Commit in response to any configure from the floating change.
    let window = f.client(id).window(&surface);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Request a size change to a different size.
    f.niri().layout.set_column_width(SizeChange::SetFixed(500));

    f.double_roundtrip(id);

    // This should request the new size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    // Before the window has a chance to respond, request a size change to the same, new size.
    f.niri().layout.set_column_width(SizeChange::SetFixed(500));

    // And also drop the Activated state to have some pending change.
    f.niri_focus_output(2);

    f.double_roundtrip(id);

    // This should keep requesting the new size (500 × 100) since the window has not responded to
    // it yet.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: []"
    );

    // Commit in response to the size change request.
    let window = f.client(id).window(&surface);
    window.set_size(300, 300);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // And also add the Activated state to have some pending change.
    f.niri_focus_output(1);

    f.double_roundtrip(id);

    // This should request the current window size (300 × 300) since the window has committed in
    // response to the size change.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 300, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn restore_floating_size() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size while we're floating and commit in response to the floating configure.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Change back to tiling.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // We should get a tiling size configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    // Resize as requested.
    let window = f.client(id).window(&surface);
    let (_, configure) = window.configures_received.last().unwrap();
    window.set_size(configure.size.0 as u16, configure.size.1 as u16);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Change back to floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // We should get a configure restoring out previous 200 × 200 size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn moving_across_workspaces_doesnt_cancel_resize() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size while we're floating and commit in response to the floating configure.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Request a size change to a different size.
    f.niri().layout.set_column_width(SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    // This should request the new size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 200, bounds: 1920 × 1080, states: [Activated]"
    );

    // Move to a different workspace before the window has a chance to respond. This will remove it
    // from one floating layout and add into a different one, potentially causing a size request.
    f.niri().layout.move_to_workspace_down(true);
    // Drop the Activated state to force a configure.
    f.niri_focus_output(2);
    f.double_roundtrip(id);

    // This should request the new size again (500 × 200) since the window hasn't responded to it.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 200, bounds: 1920 × 1080, states: []"
    );

    // Respond to the resize with a different size.
    let window = f.client(id).window(&surface);
    window.set_size(300, 300);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Focus, adding Activated, and move to workspace down, causing removing and adding to a
    // floating layout.
    f.niri_focus_output(1);
    f.niri().layout.move_to_workspace_down(true);
    f.double_roundtrip(id);

    // This should request the current size (300 × 300) since the window responded to the change.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 300, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn moving_to_floating_doesnt_cancel_resize() {
    let (mut f, id, surface) = set_up();
    let _ = f.client(id).window(&surface).recent_configures();

    // Request a size change to a different size.
    f.niri().layout.set_column_width(SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    // This should request the new size (500 ×).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    // Before the window has a chance to respond, make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This should keep requesting the new size (500 ×).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn interactive_move_unfullscreen_to_floating_restores_size() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size while we're floating and commit.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let niri = f.niri();
    let mapped = niri.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    niri.layout.set_fullscreen(&window, true);
    f.double_roundtrip(id);

    // This should request a fullscreen size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );

    // Start an interactive move which causes an unfullscreen into floating.
    let output = f.niri_output(1);
    let niri = f.niri();
    let mapped = niri.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    niri.layout
        .interactive_move_begin(window.clone(), &output, Point::default());
    niri.layout.interactive_move_update(
        &window,
        Point::from((1000., 0.)),
        output,
        Point::default(),
    );
    f.double_roundtrip(id);

    // This should request the stored floating size (200 × 200).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn interactive_move_unmaximize_to_floating_restores_size() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size while we're floating and commit.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let niri = f.niri();
    let mapped = niri.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    niri.layout.set_maximized(&window, true);
    f.double_roundtrip(id);

    // This should request a maximized size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Maximized]"
    );

    // Start an interactive move which causes an unmaximize into floating.
    let output = f.niri_output(1);
    let niri = f.niri();
    let mapped = niri.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    niri.layout
        .interactive_move_begin(window.clone(), &output, Point::default());
    niri.layout.interactive_move_update(
        &window,
        Point::from((1000., 0.)),
        output,
        Point::default(),
    );
    f.double_roundtrip(id);

    // This should request the stored floating size (200 × 200).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_during_interactive_move_propagates_to_floating() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size while we're floating and commit.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Start an interactive move.
    let output = f.niri_output(1);
    let niri = f.niri();
    let mapped = niri.layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();
    niri.layout
        .interactive_move_begin(window_id.clone(), &output, Point::default());
    niri.layout.interactive_move_update(
        &window_id,
        Point::from((1000., 0.)),
        output,
        Point::default(),
    );
    f.double_roundtrip(id);

    // This shouldn't request any new size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    // Change size while we're being interactively moved.
    let window = f.client(id).window(&surface);
    window.set_size(300, 300);
    window.commit();
    f.double_roundtrip(id);

    // This shouldn't request any new size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    // End the interactive move, placing the window into floating.
    f.niri().layout.interactive_move_end(&window_id);
    f.double_roundtrip(id);

    // This should keep the new 300 × 300 size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 300, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_in_steps() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    // Commit in response to the floating bounds state change configure.
    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);

    // Request a size change to a different size in two steps.
    f.niri().layout.set_column_width(SizeChange::SetFixed(500));
    f.niri()
        .layout
        .set_window_height(None, SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    // This should request the full new size (500 × 500) once.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 500, bounds: 1920 × 1080, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    let serial = window.configures_received.last().unwrap().0;

    // Request a size change now that the previous one is pending-but-not-acked.
    f.niri().layout.set_column_width(SizeChange::SetFixed(600));
    // Drop Activated to work around resize throttling.
    f.niri_focus_output(2);
    f.double_roundtrip(id);

    // This should request the new size (600 × 500) once.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 600 × 500, bounds: 1920 × 1080, states: []"
    );

    // Commit in response to the previous configure.
    let window = f.client(id).window(&surface);
    window.xdg_surface.ack_configure(serial);
    window.set_size(500, 500);
    window.commit();

    f.double_roundtrip(id);

    // This shouldn't request anything.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    // Request a height change now that the first one is committed-to, but the second isn't.
    let niri = f.niri();
    let mapped = niri.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    f.niri()
        .layout
        .set_window_height(Some(&window), SizeChange::SetFixed(600));
    // Add Activated to work around resize throttling.
    f.niri_focus_output(1);
    f.double_roundtrip(id);

    // This should request the latest sizes (600 × 600).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 600 × 600, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn state_change_doesnt_break_use_window_size() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    // Commit in response to the bounds change that comes with toggling floating.
    f.client(id).window(&surface).ack_last_and_commit();
    f.roundtrip(id);

    // Request a size change to a different size.
    f.niri().layout.set_column_width(SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    // This should request the new size (500 × 100).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    let serial = window.configures_received.last().unwrap().0;

    // Request a state change by dropping Activated.
    f.niri_focus_output(2);
    f.double_roundtrip(id);

    // This should request the new size (500 × 100).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: []"
    );

    // Commit in response to the previous configure with a different size.
    let window = f.client(id).window(&surface);
    window.xdg_surface.ack_configure(serial);
    window.set_size(300, 300);
    window.commit();

    f.double_roundtrip(id);

    // This shouldn't request anything.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    // Request a height change now that the first one is committed-to, but the second isn't.
    let niri = f.niri();
    let mapped = niri.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    f.niri()
        .layout
        .set_window_height(Some(&window), SizeChange::SetFixed(600));
    // Add Activated state to force a configure.
    f.niri_focus_output(1);
    f.double_roundtrip(id);

    // This should already request the current width (300 × 600) rather than the pending previous
    // width (500 × 600) from the state change configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 600, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn interactive_move_restores_floating_size_when_set_to_floating() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size while we're floating and commit to make niri remember it.
    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Change back to tiling.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // We should get a tiled size configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    // Resize as requested.
    let window = f.client(id).window(&surface);
    let (_, configure) = window.configures_received.last().unwrap();
    window.set_size(configure.size.0 as u16, configure.size.1 as u16);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Start an interactive move.
    let output = f.niri_output(1);
    let niri = f.niri();
    let mapped = niri.layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();
    niri.layout
        .interactive_move_begin(window_id.clone(), &output, Point::default());
    niri.layout.interactive_move_update(
        &window_id,
        Point::from((1000., 0.)),
        output,
        Point::default(),
    );
    f.double_roundtrip(id);

    // This shouldn't request any new size because interactive move targets tiling.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );

    // Change interactive move to target floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This should restore the floating window size (200 × 200).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );

    // End the interactive move, placing the window into floating.
    f.niri().layout.interactive_move_end(&window_id);
    f.double_roundtrip(id);

    // This should keep the floating window size (200 × 200).
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );
}

#[test]
fn floating_doesnt_store_fullscreen_size() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.add_output(2, (1280, 720));

    // Open a window fullscreen.
    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.set_fullscreen(None);
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This should request 0 × 0 to unfullscreen.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 0 × 0, bounds: 1920 × 1080, states: [Activated]"
    );

    // Without committing, make it tiling again. We never committed while floating, so there's no
    // floating size to remember.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This should request the tiled size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    // Commit in response.
    let window = f.client(id).window(&surface);
    window.set_size(100, 100);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Make the window floating again.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This shouldn't request any size change, particularly not the fullscreen size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 100 × 100, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn floating_doesnt_store_maximized_size() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.add_output(2, (1280, 720));

    // Open a window maximized.
    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.set_maximized();
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This should request 0 × 0 to unmaximize.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 0 × 0, bounds: 1920 × 1080, states: [Activated]"
    );

    // Without committing, make it tiling again. We never committed while floating, so there's no
    // floating size to remember.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This should request the tiled size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    // Commit in response.
    let window = f.client(id).window(&surface);
    window.set_size(100, 100);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Make the window floating again.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This shouldn't request any size change, particularly not the maximized size.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 100 × 100, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn floating_respects_non_fixed_min_max_rule() {
    let config = r##"
window-rule {
    min-width 200
    max-width 300
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let (mut f, id, surface) = set_up_with_config(config);

    // Commit to the Activated state configure.
    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This should clamp to min-width and request 200 × 100.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    // Commit with a bigger width than max.
    let window = f.client(id).window(&surface);
    window.set_size(400, 100);
    window.ack_last_and_commit();
    f.roundtrip(id);

    // Make it tiling.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    f.client(id).window(&surface).ack_last_and_commit();
    f.roundtrip(id);

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // This should clamp to max-width and request 300 × 100.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 100, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unmap_from_floating() {
    let (mut f, id, surface) = set_up();

    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    // Resize to something different on both axes.
    let window = f.client(id).window(&surface);
    window.attach_null();
    window.commit();

    // Shouldn't panic.
    f.double_roundtrip(id);
}

#[test]
fn unfullscreen_to_floating_doesnt_send_extra_configure() {
    let (mut f, id, surface) = set_up();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.roundtrip(id);

    // Fullscreen.
    let window = f.client(id).window(&surface);
    window.set_fullscreen(None);
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Unfullscreen via the window request which requires a configure response.
    let window = f.client(id).window(&surface);
    window.unset_fullscreen();
    f.double_roundtrip(id);

    // This should configure only once and not twice.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 936 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unmaximize_to_floating_doesnt_send_extra_configure() {
    let (mut f, id, surface) = set_up();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.roundtrip(id);

    // Maximize.
    let window = f.client(id).window(&surface);
    window.set_maximized();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Unmaximzie via the window request which requires a configure response.
    let window = f.client(id).window(&surface);
    window.unset_maximized();
    f.double_roundtrip(id);

    // This should configure only once and not twice.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 936 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unfullscreen_to_same_size_floating() {
    let (mut f, id, surface) = set_up();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size to the same as fullscreen, make niri remember it.
    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Fullscreen.
    let window = f.client(id).window(&surface);
    window.set_fullscreen(None);
    f.double_roundtrip(id);

    // The fullscreen configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );

    // Unfullscreen into floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // We should see a configure with the same size and no Fullscreen state.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unmaximize_to_same_size_floating() {
    let (mut f, id, surface) = set_up();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size to the same as maximized, make niri remember it.
    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Maximize.
    let window = f.client(id).window(&surface);
    window.set_maximized();
    f.double_roundtrip(id);

    // The maximize configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Maximized]"
    );

    // Unmaximize into floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // We should see a configure with the same size and no maximized state.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unfullscreen_to_same_size_windowed_fullscreen_floating() {
    let (mut f, id, surface) = set_up();

    let mapped = f.niri().layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size to the same as fullscreen, make niri remember it.
    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Fullscreen.
    let window = f.client(id).window(&surface);
    window.set_fullscreen(None);
    f.double_roundtrip(id);

    // The fullscreen configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );

    // Unfullscreen into windowed-fullscreen floating.
    f.niri().layout.toggle_windowed_fullscreen(&window_id);
    f.double_roundtrip(id);

    // Should send configure because the bounds have changed.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated, Fullscreen]"
    );
}

#[test]
fn unmaximize_to_same_size_windowed_fullscreen_floating() {
    let (mut f, id, surface) = set_up();

    let mapped = f.niri().layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size to the same as maximized, make niri remember it.
    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Maximize.
    let window = f.client(id).window(&surface);
    window.set_maximized();
    f.double_roundtrip(id);

    // The maximize configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Maximized]"
    );

    // Enable windowed-fullscreen.
    f.niri().layout.toggle_windowed_fullscreen(&window_id);
    f.double_roundtrip(id);

    // The windowed-fullscreen configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );

    // Go back to windowed-fullscreen floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Should send configure because the bounds have changed.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated, Fullscreen]"
    );

    // Disable windowed-fullscreen.
    f.niri().layout.toggle_windowed_fullscreen(&window_id);
    f.double_roundtrip(id);

    // Should send configure dropping the Fullscreen state.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unfullscreen_to_same_size_same_bounds_floating() {
    let config = r##"
layout {
    gaps 0
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let (mut f, id, surface) = set_up_with_config(config);

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size to the same as fullscreen, make niri remember it.
    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Fullscreen.
    let window = f.client(id).window(&surface);
    window.set_fullscreen(None);
    f.double_roundtrip(id);

    // The fullscreen configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated, Fullscreen]"
    );

    // Unfullscreen into floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // We should see a configure with the same size and no Fullscreen state.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unmaximize_to_same_size_same_bounds_floating() {
    let config = r##"
layout {
    gaps 0
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let (mut f, id, surface) = set_up_with_config(config);

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Change size to the same as fullscreen, make niri remember it.
    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    // Maximize.
    let window = f.client(id).window(&surface);
    window.set_maximized();
    f.double_roundtrip(id);

    // The maximize configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated, Maximized]"
    );

    // Unmaximize into floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // We should see a configure with the same size and no Maximized state.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn repeated_size_request() {
    let (mut f, id, surface) = set_up();
    let _ = f.client(id).window(&surface).recent_configures();

    // Make it floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // The floating configure.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 936 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );

    // Request a different width (200x100).
    f.niri()
        .layout
        .set_window_width(None, SizeChange::SetFixed(200));
    f.niri()
        .layout
        .set_window_height(None, SizeChange::SetFixed(100));
    f.double_roundtrip(id);

    // The 200x100 request.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    // Request a size change to the same size as we have just requested.
    f.niri().layout.set_column_width(SizeChange::SetFixed(200));
    f.double_roundtrip(id);

    // Should request nothing as this is a repeated same-size request in floating and the surface
    // hasn't committed to it yet.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    // Ack but don't commit yet.
    let window = f.client(id).window(&surface);
    window.ack_last();
    f.double_roundtrip(id);

    // Request a size change to the same size as we have just requested.
    f.niri().layout.set_column_width(SizeChange::SetFixed(200));
    f.double_roundtrip(id);

    // Should request nothing as this is a repeated same-size request in floating and the surface
    // hasn't committed to it yet.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    // Commit.
    let window = f.client(id).window(&surface);
    window.commit();
    f.double_roundtrip(id);

    // Request the size change again.
    f.niri().layout.set_column_width(SizeChange::SetFixed(200));
    f.double_roundtrip(id);

    // This should send a new configure since the window had committed.
    //
    // FIXME: doesn't request that currently.
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );
}

#[test]
fn floating_position_and_size_remembered_across_reopen() {
    let config = r##"
window-rule {
    open-floating true
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let mut f = Fixture::with_config(config);
    f.add_output(1, (1920, 1080));

    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.set_app_id("remembered");
    window.set_title("main");
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(300, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    f.niri().layout.move_floating_window(
        None,
        niri_ipc::PositionChange::SetFixed(500.),
        niri_ipc::PositionChange::SetFixed(400.),
        false,
    );

    let window = f.client(id).window(&surface);
    window.attach_null();
    window.commit();
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_app_id("remembered");
    window.set_title("main");
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    assert_snapshot!(
        window.format_recent_configures(),
        @"size: 300 × 200, bounds: 1920 × 1080, states: []"
    );

    window.attach_new_buffer();
    window.set_size(300, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let mut pos = None;
    f.niri().layout.with_windows(|_, _, _, layout| {
        pos = layout.tile_pos_in_workspace_view;
    });
    assert_eq!(pos, Some((500., 400.)));
}

#[test]
fn window_rules_win_over_remembered_floating_geometry() {
    let config = r##"
window-rule {
    open-floating true
    default-column-width { fixed 400; }
    default-floating-position x=10 y=20 relative-to="top-left"
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let mut f = Fixture::with_config(config);
    f.add_output(1, (1920, 1080));

    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.set_app_id("remembered");
    window.set_title("main");
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(300, 250);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    f.niri().layout.move_floating_window(
        None,
        niri_ipc::PositionChange::SetFixed(500.),
        niri_ipc::PositionChange::SetFixed(400.),
        false,
    );

    let window = f.client(id).window(&surface);
    window.attach_null();
    window.commit();
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_app_id("remembered");
    window.set_title("main");
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    assert_snapshot!(
        window.format_recent_configures(),
        @"size: 400 × 250, bounds: 1920 × 1080, states: []"
    );

    window.attach_new_buffer();
    window.set_size(400, 250);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let mut pos = None;
    f.niri().layout.with_windows(|_, _, _, layout| {
        pos = layout.tile_pos_in_workspace_view;
    });
    assert_eq!(pos, Some((10., 20.)));
}

// Opens a 300×200 window that always has the same app ID and title.
fn open_remembered_window(f: &mut Fixture, id: ClientId, surface: &WlSurface) {
    let window = f.client(id).window(surface);
    window.set_app_id("remembered");
    window.set_title("main");
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(surface);
    window.attach_new_buffer();
    window.set_size(300, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
}

fn close_window(f: &mut Fixture, id: ClientId, surface: &WlSurface) {
    let window = f.client(id).window(surface);
    window.attach_null();
    window.commit();
    f.double_roundtrip(id);
}

fn only_window_is_floating(f: &mut Fixture) -> bool {
    let ws = f.niri().layout.active_workspace().unwrap();
    let mapped = ws.windows().next().unwrap();
    ws.is_floating(&mapped.window)
}

fn only_window(f: &mut Fixture) -> smithay::desktop::Window {
    let ws = f.niri().layout.active_workspace().unwrap();
    ws.windows().next().unwrap().window.clone()
}

// Sets up a fixture with one output and the remembered window open.
fn fresh() -> (Fixture, ClientId, WlSurface) {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    let id = f.add_client();
    let surface = f.client(id).create_window().surface.clone();
    open_remembered_window(&mut f, id, &surface);
    (f, id, surface)
}

// Starts an interactive move and drags the window past the start threshold, without ending it.
fn start_drag(f: &mut Fixture, window: &smithay::desktop::Window) {
    let from = (100., 100.);
    let to = (400., 400.);
    let output = f.niri_output(1);
    assert!(f
        .niri()
        .layout
        .interactive_move_begin(window.clone(), &output, Point::from(from)));
    f.niri().layout.interactive_move_update(
        window,
        Point::from((to.0 - from.0, to.1 - from.1)),
        output,
        Point::from(to),
    );
}

#[test]
fn floating_state_remembered_across_reopen() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));

    let id = f.add_client();
    let surface = f.client(id).create_window().surface.clone();

    // With nothing remembered the window should open tiled.
    open_remembered_window(&mut f, id, &surface);
    assert!(!only_window_is_floating(&mut f));

    // Reopen it without toggling anything. It should still open tiled.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(!only_window_is_floating(&mut f));

    // Toggle to floating and reopen. It should open floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));

    // Reopen it again. It should keep opening floating.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));

    // Toggle back to tiling and reopen. It should open tiled.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(!only_window_is_floating(&mut f));
}

#[test]
fn open_floating_rule_wins_over_remembered_floating_state() {
    let config = r##"
window-rule {
    open-floating false
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let mut f = Fixture::with_config(config);
    f.add_output(1, (1920, 1080));

    let id = f.add_client();
    let surface = f.client(id).create_window().surface.clone();

    // Open the window and toggle it to floating.
    open_remembered_window(&mut f, id, &surface);
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    assert!(only_window_is_floating(&mut f));

    // Reopen it. The window rule should win over the remembered floating state.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(!only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_closed_while_fullscreen() {
    let (mut f, id, surface) = fresh();

    // Toggle to floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Fullscreen the window and commit in response.
    let w = only_window(&mut f);
    f.niri().layout.set_fullscreen(&w, true);
    f.double_roundtrip(id);
    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);

    // Close it while fullscreen and reopen. It should open floating.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_closed_while_maximized() {
    let (mut f, id, surface) = fresh();

    // Toggle to floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Maximize the window and commit in response.
    let w = only_window(&mut f);
    f.niri().layout.set_maximized(&w, true);
    f.double_roundtrip(id);
    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);

    // Close it while maximized and reopen. It should open floating.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_remembered_floating_closed_while_fullscreen() {
    let (mut f, id, surface) = fresh();

    // Toggle to floating and reopen, so that the window opens floating from memory.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));

    // Fullscreen it, then close and reopen. It should still open floating.
    let w = only_window(&mut f);
    f.niri().layout.set_fullscreen(&w, true);
    f.double_roundtrip(id);
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_floating_closed_mid_drag() {
    let (mut f, id, surface) = fresh();

    // Toggle to floating and reopen, so that the window opens floating from memory.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));

    // Start dragging it, then close it mid-drag and reopen. It should still open floating.
    let w = only_window(&mut f);
    start_drag(&mut f, &w);
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_tiled_closed_mid_drag() {
    let (mut f, id, surface) = fresh();

    // Start dragging the tiled window.
    let w = only_window(&mut f);
    start_drag(&mut f, &w);

    // Close it mid-drag and reopen. The drag alone shouldn't make it open floating.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(!only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_toggled_during_drag() {
    let (mut f, id, surface) = fresh();

    // Start dragging the tiled window.
    let w = only_window(&mut f);
    start_drag(&mut f, &w);

    // Toggle it to floating mid-drag, then drop it.
    f.niri().layout.toggle_window_floating(None);
    f.niri().layout.interactive_move_end(&w);
    f.double_roundtrip(id);
    assert!(only_window_is_floating(&mut f));

    // Reopen it. It should open floating.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_toggled_during_drag_closed_mid_drag() {
    let (mut f, id, surface) = fresh();

    // Start dragging the tiled window.
    let w = only_window(&mut f);
    start_drag(&mut f, &w);

    // Toggle it to floating and close it without ending the drag.
    f.niri().layout.toggle_window_floating(None);
    close_window(&mut f, id, &surface);

    // It should reopen floating.
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_title_changed_after_open() {
    let (mut f, id, surface) = fresh();

    // Change the title after the window has opened.
    let window = f.client(id).window(&surface);
    window.set_title("something else entirely");
    window.commit();
    f.double_roundtrip(id);

    // Toggle to floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Reopen it. It's remembered under the title it opened with, so it should open floating.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_fullscreen_then_tiled_is_remembered_tiled() {
    let (mut f, id, surface) = fresh();

    // Toggle to floating and reopen, so that the window opens floating from memory.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(only_window_is_floating(&mut f));

    // Fullscreen and unfullscreen it. It should return to floating.
    let w = only_window(&mut f);
    f.niri().layout.set_fullscreen(&w, true);
    f.double_roundtrip(id);
    f.niri().layout.set_fullscreen(&w, false);
    f.double_roundtrip(id);
    assert!(only_window_is_floating(&mut f));

    // Toggle to tiling.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    assert!(!only_window_is_floating(&mut f));

    // Reopen it. It should open tiled.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(!only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_tiled_fullscreen_stays_tiled() {
    let (mut f, id, surface) = fresh();

    // Fullscreen the tiled window.
    let w = only_window(&mut f);
    f.niri().layout.set_fullscreen(&w, true);
    f.double_roundtrip(id);

    // Close it while fullscreen and reopen. It should open tiled.
    close_window(&mut f, id, &surface);
    open_remembered_window(&mut f, id, &surface);
    assert!(!only_window_is_floating(&mut f));
}

#[test]
fn floating_memory_remember_all_at_exit() {
    let (mut f, id, _surface) = fresh();

    // Toggle to floating.
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    // Remember all windows without closing them, like niri does on exit.
    let niri = f.niri();
    niri.floating_memory.remember_all(&mut niri.layout);

    // Open the same window from a new client. It should open floating.
    let id2 = f.add_client();
    let surface2 = f.client(id2).create_window().surface.clone();
    open_remembered_window(&mut f, id2, &surface2);
    assert!(only_window_is_floating(&mut f));
}

fn floating_fixture(extra_rules: &str) -> (Fixture, ClientId, WlSurface) {
    let config = format!("window-rule {{ open-floating true; }}\n{extra_rules}");
    let config = Config::parse_mem(&config).unwrap();
    let mut f = Fixture::with_config(config);
    f.add_output(1, (1920, 1080));
    let id = f.add_client();
    let surface = f.client(id).create_window().surface.clone();
    (f, id, surface)
}

fn move_only_window_to(f: &mut Fixture, x: f64, y: f64) {
    f.niri().layout.move_floating_window(
        None,
        niri_ipc::PositionChange::SetFixed(x),
        niri_ipc::PositionChange::SetFixed(y),
        false,
    );
}

fn only_window_pos(f: &mut Fixture) -> Option<(f64, f64)> {
    let mut pos = None;
    f.niri().layout.with_windows(|_, _, _, layout| {
        pos = layout.tile_pos_in_workspace_view;
    });
    pos
}

// Sends the initial commit of the remembered window and returns its first configure.
fn initial_configure_of_remembered_window(
    f: &mut Fixture,
    id: ClientId,
    surface: &WlSurface,
) -> String {
    let _ = f.client(id).window(surface).recent_configures();
    let window = f.client(id).window(surface);
    window.set_app_id("remembered");
    window.set_title("main");
    window.commit();
    f.roundtrip(id);
    f.client(id).window(surface).format_recent_configures()
}

#[test]
fn floating_memory_untouched_window_is_not_remembered() {
    let (mut f, id, surface) = floating_fixture("");

    // Open the window and move it.
    open_remembered_window(&mut f, id, &surface);
    move_only_window_to(&mut f, 500., 400.);
    close_window(&mut f, id, &surface);

    // Open a window with the same app ID and title that picks its own size, and close it without
    // moving or resizing it.
    initial_configure_of_remembered_window(&mut f, id, &surface);
    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(100, 80);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
    close_window(&mut f, id, &surface);

    // Reopen it. It should get the geometry of the window that was moved.
    assert_snapshot!(
        initial_configure_of_remembered_window(&mut f, id, &surface),
        @"size: 300 × 200, bounds: 1920 × 1080, states: []"
    );
    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(300, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
    assert_eq!(only_window_pos(&mut f), Some((500., 400.)));
}

#[test]
fn floating_memory_resize_is_remembered() {
    let (mut f, id, surface) = floating_fixture("");

    // Open the window and resize it.
    open_remembered_window(&mut f, id, &surface);
    f.niri()
        .layout
        .set_window_width(None, SizeChange::SetFixed(500));
    f.double_roundtrip(id);
    let window = f.client(id).window(&surface);
    window.set_size(500, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
    close_window(&mut f, id, &surface);

    // Reopen it. It should get the new size.
    assert_snapshot!(
        initial_configure_of_remembered_window(&mut f, id, &surface),
        @"size: 500 × 200, bounds: 1920 × 1080, states: []"
    );
}

#[test]
fn remember_floating_geometry_false_does_not_remember() {
    let (mut f, id, surface) = floating_fixture(
        r#"window-rule { match app-id="remembered"; remember-floating-geometry false; }"#,
    );

    // Open the window and move it.
    open_remembered_window(&mut f, id, &surface);
    move_only_window_to(&mut f, 500., 400.);
    close_window(&mut f, id, &surface);

    // Reopen it. Nothing should be remembered.
    assert_snapshot!(
        initial_configure_of_remembered_window(&mut f, id, &surface),
        @"size: 0 × 0, bounds: 1920 × 1080, states: []"
    );
}

#[test]
fn remember_floating_geometry_false_does_not_restore() {
    let (mut f, id, surface) = floating_fixture("");

    // Open the window and move it.
    open_remembered_window(&mut f, id, &surface);
    move_only_window_to(&mut f, 500., 400.);
    close_window(&mut f, id, &surface);

    // Turn off remembering for the window and reopen it. The remembered geometry shouldn't apply.
    let config = Config::parse_mem(
        r#"
window-rule { open-floating true; }
window-rule { match app-id="remembered"; remember-floating-geometry false; }
"#,
    )
    .unwrap();
    f.niri_state().reload_config(Ok(config));
    assert_snapshot!(
        initial_configure_of_remembered_window(&mut f, id, &surface),
        @"size: 0 × 0, bounds: 1920 × 1080, states: []"
    );
    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(300, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
    assert_ne!(only_window_pos(&mut f), Some((500., 400.)));
}

#[test]
fn is_first_of_app_matcher() {
    let config = r##"
window-rule {
    match app-id="^browser$"
    open-floating true
}

window-rule {
    match app-id="^browser$" is-first-of-app=true
    open-floating false
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let mut f = Fixture::with_config(config);
    f.add_output(1, (1920, 1080));
    let id = f.add_client();

    // Opens a 300×200 window with the given app ID.
    let open = |f: &mut Fixture, app_id: &str| {
        let surface = f.client(id).create_window().surface.clone();
        let window = f.client(id).window(&surface);
        window.set_app_id(app_id);
        window.commit();
        f.roundtrip(id);

        let window = f.client(id).window(&surface);
        window.attach_new_buffer();
        window.set_size(300, 200);
        window.ack_last_and_commit();
        f.double_roundtrip(id);
        surface
    };
    // Returns whether each window is floating, in the order they were opened.
    let floating = |f: &mut Fixture| -> Vec<bool> {
        let ws = f.niri().layout.active_workspace().unwrap();
        let mut rv: Vec<_> = ws
            .windows()
            .map(|mapped| (mapped.id().get(), ws.is_floating(&mapped.window)))
            .collect();
        rv.sort();
        rv.into_iter().map(|(_, floating)| floating).collect()
    };

    // The first browser window should open tiled.
    let main = open(&mut f, "browser");
    assert_eq!(floating(&mut f), [false]);

    // A window of another app doesn't count, so the second browser window should open floating.
    let _other = open(&mut f, "other");
    let popup = open(&mut f, "browser");
    assert_eq!(floating(&mut f), [false, false, true]);

    // Close the first browser window. Another one is still open, so a new one should open floating.
    close_window(&mut f, id, &main);
    let second_popup = open(&mut f, "browser");
    assert_eq!(floating(&mut f), [false, true, true]);

    // Close all browser windows. The next one is the first again and should open tiled.
    close_window(&mut f, id, &popup);
    close_window(&mut f, id, &second_popup);
    let _new_main = open(&mut f, "browser");
    assert_eq!(floating(&mut f), [false, false]);
}
