use core::sync::atomic::{AtomicU64, Ordering};

static UPTIME_NS: AtomicU64 = AtomicU64::new(0);
static PIC_TIMER_PERIOD_NS: AtomicU64 = AtomicU64::new(0);
static LOCAL_TIMER_PERIOD_NS: AtomicU64 = AtomicU64::new(0);

pub fn set_pic_timer_period_ns(period_ns: u64) {
    PIC_TIMER_PERIOD_NS.store(period_ns, Ordering::Relaxed);
}

pub fn set_local_timer_period_ns(period_ns: u64) {
    LOCAL_TIMER_PERIOD_NS.store(period_ns, Ordering::Relaxed);
}

pub fn tick_pic_timer() {
    tick_ns(PIC_TIMER_PERIOD_NS.load(Ordering::Relaxed));
}

pub fn tick_local_timer() {
    tick_ns(LOCAL_TIMER_PERIOD_NS.load(Ordering::Relaxed));
}

pub fn tick_ns(delta_ns: u64) {
    UPTIME_NS.fetch_add(delta_ns, Ordering::Relaxed);
}

pub fn uptime_ns() -> u64 {
    UPTIME_NS
        .load(Ordering::Relaxed)
        .saturating_add(crate::arch::timer_offset_ns())
}
