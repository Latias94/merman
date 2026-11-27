#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalValidationWork {
    pub calls: usize,
    pub svg_bytes: usize,
    pub xml_walks: usize,
    pub xml_bytes: usize,
}

thread_local! {
    static TERMINAL_VALIDATION_WORK: std::cell::Cell<Option<TerminalValidationWork>> = const {
        std::cell::Cell::new(None)
    };
}

pub fn measure_terminal_validation_work<T>(
    operation: impl FnOnce() -> T,
) -> (T, TerminalValidationWork) {
    struct Restore(Option<TerminalValidationWork>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TERMINAL_VALIDATION_WORK.set(self.0);
        }
    }

    let _restore = Restore(TERMINAL_VALIDATION_WORK.replace(Some(Default::default())));
    let output = operation();
    let work = TERMINAL_VALIDATION_WORK
        .get()
        .expect("terminal validation scope is active");
    (output, work)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XmlValidationWork {
    pub calls: usize,
    pub svg_bytes: usize,
}

thread_local! {
    static XML_VALIDATION_WORK: std::cell::Cell<Option<XmlValidationWork>> = const {
        std::cell::Cell::new(None)
    };
}

pub fn measure_xml_validation_work<T>(operation: impl FnOnce() -> T) -> (T, XmlValidationWork) {
    struct Restore(Option<XmlValidationWork>);
    impl Drop for Restore {
        fn drop(&mut self) {
            XML_VALIDATION_WORK.set(self.0);
        }
    }

    let _restore = Restore(XML_VALIDATION_WORK.replace(Some(Default::default())));
    let output = operation();
    let work = XML_VALIDATION_WORK
        .get()
        .expect("XML validation scope is active");
    (output, work)
}

pub(super) fn record_terminal_xml_walk(svg: &str) {
    TERMINAL_VALIDATION_WORK.with(|counter| {
        counter.set(counter.get().map(|mut work| {
            work.xml_walks += 1;
            work.xml_bytes += svg.len();
            work
        }));
    });
}

pub(super) fn record_xml_walk(svg: &str) {
    record_terminal_xml_walk(svg);
    XML_VALIDATION_WORK.with(|counter| {
        counter.set(counter.get().map(|mut work| {
            work.calls += 1;
            work.svg_bytes += svg.len();
            work
        }));
    });
}

pub(super) fn record_terminal_validation(svg: &str) {
    TERMINAL_VALIDATION_WORK.with(|counter| {
        counter.set(counter.get().map(|mut work| {
            work.calls += 1;
            work.svg_bytes += svg.len();
            work
        }));
    });
}
