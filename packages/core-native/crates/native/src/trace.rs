use std::{
    cell::Cell,
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use once_cell::sync::Lazy;

use crate::{api::QtTraceRecord, qt};

const MAX_TRACE_RECORDS: usize = 8192;

#[derive(Debug, Clone)]
struct TraceRecordInternal {
    trace_id: u64,
    ts_us: u64,
    lane: String,
    stage: String,
    node_id: Option<u32>,
    listener_id: Option<u16>,
    prop_id: Option<u16>,
    detail: Option<String>,
}

static TRACE_ENABLED: AtomicBool = AtomicBool::new(false);
static NEXT_TRACE_ID: AtomicU64 = AtomicU64::new(1);
static TRACE_RECORDS: Lazy<Mutex<VecDeque<TraceRecordInternal>>> =
    Lazy::new(|| Mutex::new(VecDeque::with_capacity(MAX_TRACE_RECORDS)));

thread_local! {
    static CURRENT_INTERACTION_ID: Cell<u64> = const { Cell::new(0) };
}

fn push_record(record: TraceRecordInternal) {
    let mut records = TRACE_RECORDS.lock().expect("trace buffer mutex poisoned");
    if records.len() >= MAX_TRACE_RECORDS {
        records.pop_front();
    }
    records.push_back(record);
}

pub(crate) fn set_enabled(enabled: bool) {
    TRACE_ENABLED.store(enabled, Ordering::SeqCst);
}

pub(crate) fn is_enabled() -> bool {
    TRACE_ENABLED.load(Ordering::SeqCst)
}

pub(crate) fn clear() {
    TRACE_RECORDS
        .lock()
        .expect("trace buffer mutex poisoned")
        .clear();
    NEXT_TRACE_ID.store(1, Ordering::SeqCst);
    CURRENT_INTERACTION_ID.with(|cell| cell.set(0));
}

pub(crate) fn snapshot() -> Vec<QtTraceRecord> {
    TRACE_RECORDS
        .lock()
        .expect("trace buffer mutex poisoned")
        .iter()
        .cloned()
        .map(|record| QtTraceRecord {
            trace_id: record.trace_id as i64,
            ts_us: record.ts_us as i64,
            lane: record.lane,
            stage: record.stage,
            node_id: record.node_id,
            listener_id: record.listener_id,
            prop_id: record.prop_id,
            detail: record.detail,
        })
        .collect()
}

pub(crate) fn next_trace_id() -> u64 {
    if !is_enabled() {
        return 0;
    }

    NEXT_TRACE_ID.fetch_add(1, Ordering::SeqCst)
}

pub(crate) fn enter_interaction(trace_id: u64) {
    CURRENT_INTERACTION_ID.with(|cell| cell.set(trace_id));
}

pub(crate) fn exit_interaction() {
    CURRENT_INTERACTION_ID.with(|cell| cell.set(0));
}

pub(crate) fn current_interaction_id() -> u64 {
    CURRENT_INTERACTION_ID.with(Cell::get)
}

pub(crate) fn record_static(
    trace_id: u64,
    lane: &str,
    stage: &str,
    node_id: Option<u32>,
    listener_id: Option<u16>,
    prop_id: Option<u16>,
    detail: Option<String>,
) {
    if !is_enabled() || trace_id == 0 {
        return;
    }

    push_record(TraceRecordInternal {
        trace_id,
        ts_us: qt::trace_now_ns() / 1_000,
        lane: lane.to_owned(),
        stage: stage.to_owned(),
        node_id,
        listener_id,
        prop_id,
        detail,
    });
}

pub(crate) fn record_dynamic(
    trace_id: u64,
    lane: String,
    stage: String,
    node_id: Option<u32>,
    listener_id: Option<u16>,
    prop_id: Option<u16>,
    detail: Option<String>,
) {
    if !is_enabled() || trace_id == 0 {
        return;
    }

    push_record(TraceRecordInternal {
        trace_id,
        ts_us: qt::trace_now_ns() / 1_000,
        lane,
        stage,
        node_id,
        listener_id,
        prop_id,
        detail,
    });
}

pub(crate) fn record_current_static(
    lane: &str,
    stage: &str,
    node_id: Option<u32>,
    listener_id: Option<u16>,
    prop_id: Option<u16>,
    detail: Option<String>,
) -> u64 {
    let trace_id = current_interaction_id();
    record_static(trace_id, lane, stage, node_id, listener_id, prop_id, detail);
    trace_id
}
