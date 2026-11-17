//! A47 component allocation budget; this is not whole-phone PSS or UI allocation evidence.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::atomic::{AtomicIsize, AtomicUsize, Ordering},
};
thread_local! {static TRACK:Cell<bool>=const{Cell::new(false)};}
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
#[repr(C)]
struct Header {
    tracked: usize,
    size: usize,
}
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let align = l.align().max(std::mem::align_of::<Header>());
        let offset = (std::mem::size_of::<Header>() + align - 1) & !(align - 1);
        let Some(size) = offset.checked_add(l.size()) else {
            return std::ptr::null_mut();
        };
        let Ok(total) = Layout::from_size_align(size, align) else {
            return std::ptr::null_mut();
        };
        let base = System.alloc(total);
        if base.is_null() {
            return base;
        }
        let p = base.add(offset);
        let tracked = TRACK.try_with(Cell::get).unwrap_or(false);
        p.sub(std::mem::size_of::<Header>())
            .cast::<Header>()
            .write(Header {
                tracked: tracked as usize,
                size: l.size(),
            });
        if tracked {
            let live = LIVE.fetch_add(l.size() as isize, Ordering::Relaxed) + l.size() as isize;
            PEAK.fetch_max(live as usize, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        let h = p.sub(std::mem::size_of::<Header>()).cast::<Header>().read();
        if h.tracked != 0 {
            LIVE.fetch_sub(h.size as isize, Ordering::Relaxed);
        }
        let align = l.align().max(std::mem::align_of::<Header>());
        let offset = (std::mem::size_of::<Header>() + align - 1) & !(align - 1);
        System.dealloc(
            p.sub(offset),
            Layout::from_size_align_unchecked(l.size() + offset, align),
        );
    }
}
#[global_allocator]
static ALLOC: Allocator = Allocator;
#[test]
fn catalog_and_reading_index_fit_the_1000_entry_component_budget() {
    use quiet_launcher::{
        catalog::AppCatalog,
        model::AppEntry,
        search::{AppSearch, ReadingSearch},
    };
    TRACK.with(|t| t.set(true));
    let kana = "あいうえおかきくけこ"
        .repeat(6)
        .chars()
        .take(59)
        .collect::<String>();
    let apps: Vec<_> = (0..1000)
        .map(|i| AppEntry {
            id: format!("example.perf.app{i:05}/Main"),
            label: format!("{i:05}{kana}"),
        })
        .collect();
    let mut catalog = AppCatalog::default();
    catalog.refresh("fixed:ja:1", || Ok(apps)).unwrap();
    let search = ReadingSearch::default();
    let query = format!("00500{kana}");
    assert_eq!(
        search.search(&query, &catalog.apps())[0].id,
        "example.perf.app00500/Main"
    );
    let retained = LIVE.load(Ordering::Relaxed);
    for _ in 0..100 {
        let snapshot = catalog.apps();
        assert_eq!(search.search(&query, &snapshot).len(), 1);
    }
    let repeated = LIVE.load(Ordering::Relaxed);
    let peak = PEAK.load(Ordering::Relaxed);
    TRACK.with(|t| t.set(false));
    assert!(
        retained <= 8 * 1024 * 1024,
        "component retained {retained} bytes exceeds 8 MiB"
    );
    assert!(
        peak <= 16 * 1024 * 1024,
        "component peak {peak} bytes exceeds 16 MiB"
    );
    assert!(
        repeated <= retained + 64 * 1024,
        "repeated access retained new copies: {retained} -> {repeated}"
    );
    println!("component retained_bytes={retained} peak_bytes={peak}; excludes UI indexes, JVM, fonts and total PSS");
}
