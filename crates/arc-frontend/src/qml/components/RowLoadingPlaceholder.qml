import QtQuick

// A per-row "this section is still resolving" indicator, shown in place of
// a section's cards while its app ids are still being looked up. Reuses
// the same conveyor-belt animation as the page-level LoadingOverlay so
// per-row loading still reads as "loading", not a different/lesser state.
// Sized by the caller via the usual Layout.fillWidth/Layout.preferredHeight
// attached properties, same as CardCarousel/HeroCarousel.
Item {
    id: root

    ConveyorLoader {
        anchors.centerIn: parent
    }
}
