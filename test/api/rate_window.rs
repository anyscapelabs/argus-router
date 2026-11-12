use api::services::rate::RateWindow;

#[test]
fn allows_up_to_the_quota_then_refuses() {
    let window = RateWindow::default();
    for expected in 1..=5 {
        let (used, limit) = window.hit("user-a", 5);
        assert_eq!(used, expected);
        assert_eq!(limit, 5);
    }
    let (used, _) = window.hit("user-a", 5);
    assert_eq!(used, 6, "the minute window is full past the quota");
}

#[test]
fn tracks_users_independently() {
    let window = RateWindow::default();
    for _ in 0..3 {
        window.hit("user-a", 2);
    }
    let (used, _) = window.hit("user-b", 2);
    assert_eq!(used, 1, "other users get a fresh window");
}

#[test]
fn zero_quota_means_unlimited() {
    let window = RateWindow::default();
    for _ in 0..100 {
        let (used, quota) = window.hit("user-a", 0);
        assert_eq!(used, 0);
        assert_eq!(quota, 0);
    }
}
