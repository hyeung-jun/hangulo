use std::time::{Duration, Instant};

use crate::platform;

const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// 이 간격 안에 이어진 클립보드 변화는 같은 복사로 본다. 복사 한 번에 클립보드를 여러 번
/// 건드리는 앱(비우고 → 쓰기)이 있어, 그걸 두 번 복사로 오인하지 않기 위해서다.
const SAME_COPY_GAP: Duration = Duration::from_millis(50);

/// 두 번째 복사가 이 시간 안에 오면 "복사 두 번"으로 인정한다.
const DOUBLE_COPY_WINDOW: Duration = Duration::from_millis(600);

/// 클립보드 변경 카운터의 흐름에서 "짧은 간격으로 두 번 복사"를 찾아낸다.
struct DoubleCopyDetector {
    last_count: isize,
    last_copy_at: Option<Instant>,
    /// 직전 복사가 이미 두 번째 복사로 쓰였는지. 세 번 연달아 복사했을 때 2·3번째가
    /// 다시 한 쌍으로 묶여 번역이 두 번 뜨는 것을 막는다.
    last_copy_fired: bool,
}

impl DoubleCopyDetector {
    fn new(count: isize) -> Self {
        Self {
            last_count: count,
            last_copy_at: None,
            last_copy_fired: false,
        }
    }

    fn observe(&mut self, count: isize, now: Instant) -> bool {
        if count == self.last_count {
            return false;
        }
        self.last_count = count;

        let since_last_copy = self.last_copy_at.map(|at| now.duration_since(at));
        if since_last_copy.is_some_and(|elapsed| elapsed < SAME_COPY_GAP) {
            return false;
        }

        let is_double = !self.last_copy_fired
            && since_last_copy.is_some_and(|elapsed| elapsed <= DOUBLE_COPY_WINDOW);
        self.last_copy_at = Some(now);
        self.last_copy_fired = is_double;
        is_double
    }
}

/// 클립보드를 폴링하다가 복사가 연달아 두 번 일어나면 `on_double_copy`를 부른다.
pub fn spawn_watcher(on_double_copy: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        let mut detector = DoubleCopyDetector::new(platform::clipboard_change_count());
        loop {
            std::thread::sleep(POLL_INTERVAL);
            if detector.observe(platform::clipboard_change_count(), Instant::now()) {
                // 두 번째 복사가 클립보드에 다 쓰이기 전에 읽지 않도록 잠깐 기다린다.
                std::thread::sleep(SAME_COPY_GAP);
                on_double_copy();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn 짧은_간격의_두_번_복사만_인정한다() {
        let start = Instant::now();
        let mut detector = DoubleCopyDetector::new(0);
        assert!(!detector.observe(1, at(start, 0)));
        assert!(detector.observe(2, at(start, 200)));
    }

    #[test]
    fn 한_번_복사나_느린_두_번_복사는_무시한다() {
        let start = Instant::now();
        let mut detector = DoubleCopyDetector::new(0);
        assert!(!detector.observe(1, at(start, 0)));
        assert!(!detector.observe(1, at(start, 200)));
        assert!(!detector.observe(2, at(start, 1000)));
    }

    #[test]
    fn 복사_한_번이_카운터를_여러_번_올려도_한_번으로_센다() {
        let start = Instant::now();
        let mut detector = DoubleCopyDetector::new(0);
        assert!(!detector.observe(1, at(start, 0)));
        assert!(!detector.observe(2, at(start, 25)));
        assert!(detector.observe(3, at(start, 200)));
        assert!(!detector.observe(4, at(start, 225)));
    }

    #[test]
    fn 세_번_연달아_복사해도_한_번만_인정한다() {
        let start = Instant::now();
        let mut detector = DoubleCopyDetector::new(0);
        assert!(!detector.observe(1, at(start, 0)));
        assert!(detector.observe(2, at(start, 200)));
        assert!(!detector.observe(3, at(start, 400)));
    }
}
