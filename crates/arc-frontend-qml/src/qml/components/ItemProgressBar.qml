import QtQuick.Controls as Controls

// indeterminate until the daemon reports a real percentage
Controls.ProgressBar {
    id: root

    property real progress: 0

    indeterminate: root.progress <= 0
    value: root.progress
}
