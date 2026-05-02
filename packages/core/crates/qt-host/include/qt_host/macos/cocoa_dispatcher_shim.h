#ifndef QT_SOLID_COCOA_DISPATCHER_SHIM_H
#define QT_SOLID_COCOA_DISPATCHER_SHIM_H

#include <QtCore/private/qabstracteventdispatcher_p.h>
#include <QtCore/private/qtimerinfo_unix_p.h>

// This shim only models the private prefix we actually read from
// QCocoaEventDispatcherPrivate. Qt 6.8 -> 6.9 looks low-churn for this prefix,
// but we intentionally keep the support gate narrow until each minor line is
// checked against the installed binaries we ship against.
#if QT_VERSION < QT_VERSION_CHECK(6, 10, 0) || \
    QT_VERSION >= QT_VERSION_CHECK(6, 11, 0)
#error "qt-solid-native macOS wait bridge shim supports Qt 6.10.x only"
#endif

QT_BEGIN_NAMESPACE

struct QtSolidQCocoaEventDispatcherPrivatePrefix
    : public QAbstractEventDispatcherPrivate {
  uint processEventsFlags;
  QTimerInfoList timerInfoList;
};

QT_END_NAMESPACE

#endif  // QT_SOLID_COCOA_DISPATCHER_SHIM_H
