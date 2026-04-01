#pragma once
#include <QObject>
#include <QString>

class DeviceManager : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString deviceName READ deviceName NOTIFY deviceNameChanged)
    Q_PROPERTY(int deviceCount READ deviceCount NOTIFY deviceCountChanged)
    Q_PROPERTY(bool active READ active WRITE setActive NOTIFY activeChanged)
public:
    Q_INVOKABLE void connect(const QString &address);
    Q_INVOKABLE void disconnect();
    Q_INVOKABLE int scanDevices();
signals:
    void deviceNameChanged();
    void deviceCountChanged();
    void activeChanged();
    void deviceFound(const QString &name);
};
