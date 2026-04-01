#pragma once
#include <QObject>

class SensorController : public QObject {
    Q_OBJECT
    Q_PROPERTY(double temperature READ temperature NOTIFY temperatureChanged)
    Q_PROPERTY(bool connected READ connected NOTIFY connectedChanged)
    Q_PROPERTY(int sensorCount READ sensorCount NOTIFY sensorCountChanged)
public:
    Q_INVOKABLE void calibrate();
    Q_INVOKABLE void reset();
signals:
    void temperatureChanged();
    void connectedChanged();
    void sensorCountChanged();
};
