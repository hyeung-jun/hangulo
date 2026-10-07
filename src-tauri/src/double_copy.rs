use std::time::{Duration, Instant};

use crate::platform;

const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// 이 간격 안에 이어진 클립보드 변화는 같은 복사로 본다. 복사 한 번에 클립보드를 여러 번
/// 건드리는 앱(비우고 → 쓰기)이 있어, 그걸 두 번 복사로 오인하지 않기 위해서다.
const SAME_COPY_GAP: Duration = Duration::from_millis(50);

/// 두 번째 복사가 이 시간 안에 오면 "복사 두 번"으로 인정한다.
const DOUBLE_COPY_WINDOW: Duration = Duration::from_millis(600);

/// 두 번째 복사 뒤 이만큼 클립보드가 조용해야 발동한다. 복사 키를 길게 누르면 키 반복으로
/// 복사가 계속 이어지는데, 그걸 두 번 복사로 오인하지 않기 위해서다. 두 번째 복사가
/// 클립보드에 다 쓰일 때까지 기다리는 역할도 겸한다.
const QUIET_AFTER_SECOND_COPY: Duration = Duration::from_millis(150);

/// 클립보드 변경 카운터의 흐름에서 "짧은 간격으로 딱 두 번 복사"를 찾아낸다.
struct DoubleCopyDetector {
    last_count: isize,
    last_change_at: Option<Instant>,
    last_copy_at: Option<Instant>,
    /// `DOUBLE_COPY_WINDOW` 안에 끊기지 않고 이어진 복사 횟수.
    consecutive_copies: u32,
}

impl DoubleCopyDetector {
    fn new(count: isize) -> Self {
        Self {
            last_count: count,
            last_change_at: None,
            last_copy_at: None,
            consecutive_copies: 0,
        }
    }

    /// 폴링할 때마다(카운터가 그대로여도) 불러야 한다. 발동 시점이 두 번째 복사 순간이 아니라
    /// 그 뒤 조용한 시간이 지난 때이기 때문이다.
    fn observe(&mut self, count: isize, now: Instant) -> bool {
        if count != self.last_count {
            self.last_count = count;
            self.last_change_at = Some(now);

            let since_last_copy = self.last_copy_at.map(|at| now.duration_since(at));
            if since_last_copy.is_some_and(|elapsed| elapsed < SAME_COPY_GAP) {
                return false;
            }
            let continues = since_last_copy.is_some_and(|elapsed| elapsed <= DOUBLE_COPY_WINDOW);
            self.consecutive_copies = if continues { self.consecutive_copies + 1 } else { 1 };
            self.last_copy_at = Some(now);
            return false;
        }

        let quiet = self
            .last_change_at
            .is_some_and(|at| now.duration_since(at) >= QUIET_AFTER_SECOND_COPY);
        if self.consecutive_copies == 2 && quiet {
            // 이어지는 복사에서 다시 발동하지 않도록 세 번째까지 센 것으로 친다.
            self.consecutive_copies = 3;
            return true;
        }
        false
    }
}

/// 클립보드를 폴링하다가 복사가 연달아 두 번 일어나면 `on_double_copy`를 부른다.
pub fn spawn_watcher(on_double_copy: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        let mut detector = DoubleCopyDetector::new(platform::clipboard_change_count());
        loop {
            std::thread::sleep(POLL_INTERVAL);
            if detector.observe(platform::clipboard_change_count(), Instant::now()) {
                on_double_copy();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (시각 ms, 카운터) 순서대로 관찰시키고, 발동한 시각들을 돌려준다.
    fn fired_at(observations: &[(u64, isize)]) -> Vec<u64> {
        let start = Instant::now();
        let mut detector = DoubleCopyDetector::new(0);
        observations
            .iter()
            .filter(|(ms, count)| detector.observe(*count, start + Duration::from_millis(*ms)))
            .map(|(ms, _)| *ms)
            .collect()
    }

    #[test]
    fn 두_번_복사하고_조용해지면_발동한다() {
        assert_eq!(fired_at(&[(0, 1), (200, 2), (300, 2), (350, 2), (400, 2)]), [350]);
    }

    #[test]
    fn 한_번_복사나_느린_두_번_복사는_무시한다() {
        assert_eq!(fired_at(&[(0, 1), (200, 1), (1000, 2), (1200, 2)]), [0u64; 0]);
    }

    #[test]
    fn 복사_한_번이_카운터를_여러_번_올려도_한_번으로_센다() {
        assert_eq!(fired_at(&[(0, 1), (25, 2), (300, 2)]), [0u64; 0]);
        assert_eq!(fired_at(&[(0, 1), (25, 2), (200, 3), (225, 4), (375, 4)]), [375]);
    }

    #[test]
    fn 세_번_이상_이어진_복사는_무시한다() {
        assert_eq!(fired_at(&[(0, 1), (200, 2), (300, 3), (500, 3)]), [0u64; 0]);
    }

    #[test]
    fn 복사_키를_길게_눌러_반복되는_복사는_무시한다() {
        let mut observations = vec![(0, 1)];
        observations.extend((0..40).map(|i| (225 + i * 30, 2 + i as isize)));
        observations.push((2000, 41));
        assert_eq!(fired_at(&observations), [0u64; 0]);
    }

    #[test]
    fn 발동한_뒤_이어진_복사로는_다시_발동하지_않는다() {
        assert_eq!(fired_at(&[(0, 1), (200, 2), (350, 2), (700, 3), (900, 3)]), [350]);
    }
}
