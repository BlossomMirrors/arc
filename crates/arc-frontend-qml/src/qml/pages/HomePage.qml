pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Kirigami.ScrollablePage {
    id: root

    Kirigami.ColumnView.fillWidth: true

    leftPadding: Kirigami.Units.largeSpacing * 2
    rightPadding: Kirigami.Units.largeSpacing * 2
    topPadding: Kirigami.Units.largeSpacing

    title: i18n("Home")

    Component.onCompleted: HomeFeedModel.load()

    Timer {
        id: coldStartRetry
        property int attempts: 0
        interval: 30000
        running: true
        repeat: true
        onTriggered: {
            attempts += 1;
            HomeFeedModel.refresh();
            if (attempts >= 4) {
                stop();
            }
        }
    }

    function openApp(pkgId, seed) {
        applicationWindow().openApp(pkgId, seed);
    }

    function openStory(storyId) {
        applicationWindow().openStory(storyId);
    }

    function openCategory(categoryId, categoryLabel, categoryColor, categoryIcon) {
        applicationWindow().openCategory(categoryId, categoryLabel, categoryColor, categoryIcon);
    }

    ConveyorLoader {
        anchors.centerIn: parent
        visible: HomeFeedModel.loading
    }

    ColumnLayout {
        visible: !HomeFeedModel.loading
        width: root.availableWidth
        spacing: 0

        ColumnLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 76
            spacing: 0

            Repeater {
                model: HomeFeedModel

            delegate: Loader {
                id: rowLoader

                required property int index
                required property string itemType
                required property string text
                required property string title
                required property string cardsJson
                required property string heroItemsJson
                required property string editorialItemsJson
                required property string linkTitle
                required property string linkItemsJson
                required property string categoriesJson

                Layout.fillWidth: true
                Layout.topMargin: index === 0 ? 0
                    : ["carousel", "app-row", "app-grid", "categories"].indexOf(itemType) >= 0
                        ? Kirigami.Units.gridUnit * 2.5
                        : Kirigami.Units.largeSpacing

                sourceComponent: {
                    switch (rowLoader.itemType) {
                    case "h1": return headingComponent;
                    case "h2": return headingComponent;
                    case "h3": return headingComponent;
                    case "p": return paragraphComponent;
                    case "br": return rowLoader.index === 0 ? null : spacerComponent;
                    case "categories": return categoriesComponent;
                    case "app-row": return appRowComponent;
                    case "app-grid": return appGridComponent;
                    case "carousel": return carouselComponent;
                    case "links": return linksComponent;
                    default: return null;
                    }
                }

                Component {
                    id: headingComponent
                    Kirigami.Heading {
                        level: rowLoader.itemType === "h1" ? 1 : rowLoader.itemType === "h2" ? 2 : 3
                        text: rowLoader.text
                        wrapMode: Text.WordWrap
                    }
                }

                Component {
                    id: paragraphComponent
                    Controls.Label {
                        text: rowLoader.text
                        opacity: 0.7
                        wrapMode: Text.WordWrap
                    }
                }

                Component {
                    id: spacerComponent
                    Kirigami.Separator {}
                }

                Component {
                    id: categoriesComponent
                    ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing

                        Kirigami.Heading {
                            level: 2
                            text: i18n("Categories")
                        }

                        GridLayout {
                            Layout.alignment: Qt.AlignHCenter
                            Layout.fillWidth: true
                            Layout.maximumWidth: Kirigami.Units.gridUnit * 41
                            columns: 3
                            columnSpacing: Kirigami.Units.largeSpacing
                            rowSpacing: Kirigami.Units.largeSpacing

                            Repeater {
                                model: JSON.parse(rowLoader.categoriesJson)

                                delegate: CategoryCard {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: Kirigami.Units.gridUnit * 5
                                    categoryId: modelData.id
                                    label: modelData.label
                                    iconName: modelData.icon_name
                                    bgColor: modelData.color
                                    onActivated: root.openCategory(modelData.id, modelData.label, modelData.color, modelData.icon_name)
                                }
                            }
                        }
                    }
                }

                Component {
                    id: appRowComponent
                    ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing

                        Kirigami.Heading {
                            level: 2
                            text: rowLoader.title
                            visible: text.length > 0
                        }

                        CardCarousel {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 10
                            model: JSON.parse(rowLoader.cardsJson)

                            delegate: AppCard {
                                required property var modelData
                                width: Kirigami.Units.gridUnit * 10
                                height: Kirigami.Units.gridUnit * 10
                                pkgId: modelData.id
                                appName: modelData.name
                                summary: modelData.summary
                                iconUrl: modelData.icon_url
                                installed: modelData.installed
                                onActivated: root.openApp(modelData.id, {
                                    name: modelData.name,
                                    summary: modelData.summary,
                                    iconUrl: modelData.icon_url,
                                    installed: modelData.installed
                                })
                            }
                        }
                    }
                }

                Component {
                    id: appGridComponent
                    ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing

                        Kirigami.Heading {
                            level: 2
                            text: rowLoader.title
                            visible: text.length > 0
                        }

                        GridLayout {
                            id: grid
                            Layout.fillWidth: true
                            columns: Math.max(2, Math.floor(width / (Kirigami.Units.gridUnit * 12)))
                            columnSpacing: Kirigami.Units.largeSpacing
                            rowSpacing: Kirigami.Units.largeSpacing

                            Repeater {
                                model: JSON.parse(rowLoader.cardsJson)

                                delegate: AppCard {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: Kirigami.Units.gridUnit * 10
                                    pkgId: modelData.id
                                    appName: modelData.name
                                    summary: modelData.summary
                                    iconUrl: modelData.icon_url
                                    installed: modelData.installed
                                    onActivated: root.openApp(modelData.id, {
                                        name: modelData.name,
                                        summary: modelData.summary,
                                        iconUrl: modelData.icon_url,
                                        installed: modelData.installed
                                    })
                                }
                            }
                        }
                    }
                }

                Component {
                    id: carouselComponent
                    ColumnLayout {
                        spacing: Kirigami.Units.largeSpacing

                        HeroCarousel {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 400
                            visible: items.length > 0
                            items: JSON.parse(rowLoader.heroItemsJson)
                            onStoryActivated: storyIndex => root.openStory("story-" + storyIndex)
                            onAppActivated: pkgId => root.openApp(pkgId)
                        }

                        CardCarousel {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 150
                            visible: count > 0
                            model: JSON.parse(rowLoader.editorialItemsJson)

                            delegate: HeroCard {
                                required property var modelData
                                width: 220
                                height: 150
                                bannerUrl: modelData.banner_url
                                iconUrl: modelData.icon_url
                                heroTitle: modelData.title
                                body: modelData.body
                                onActivated: modelData.is_story
                                    ? root.openStory("story-" + modelData.story_index)
                                    : root.openApp(modelData.id)
                            }
                        }

                        CardCarousel {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 10
                            visible: count > 0
                            model: JSON.parse(rowLoader.cardsJson)

                            delegate: AppCard {
                                required property var modelData
                                width: Kirigami.Units.gridUnit * 10
                                height: Kirigami.Units.gridUnit * 10
                                pkgId: modelData.id
                                appName: modelData.name
                                summary: modelData.summary
                                iconUrl: modelData.icon_url
                                installed: modelData.installed
                                onActivated: root.openApp(modelData.id, {
                                    name: modelData.name,
                                    summary: modelData.summary,
                                    iconUrl: modelData.icon_url,
                                    installed: modelData.installed
                                })
                            }
                        }
                    }
                }

                Component {
                    id: linksComponent
                    ColumnLayout {
                        spacing: 0

                        Kirigami.Heading {
                            level: 2
                            text: rowLoader.linkTitle
                            visible: text.length > 0
                            Layout.bottomMargin: Kirigami.Units.smallSpacing
                        }

                        Repeater {
                            model: JSON.parse(rowLoader.linkItemsJson)

                            delegate: ColumnLayout {
                                id: linkRow
                                required property var modelData
                                Layout.fillWidth: true
                                spacing: 0

                                Controls.ItemDelegate {
                                    Layout.fillWidth: true
                                    text: linkRow.modelData.text
                                    icon.name: linkRow.modelData.href.length > 0 ? "link-symbolic" : ""
                                    onClicked: {
                                        if (linkRow.modelData.story_index >= 0) {
                                            root.openStory("story-" + linkRow.modelData.story_index);
                                        } else if (linkRow.modelData.app_id.length > 0) {
                                            root.openApp(linkRow.modelData.app_id);
                                        } else if (linkRow.modelData.href.length > 0) {
                                            Qt.openUrlExternally(linkRow.modelData.href);
                                        }
                                    }
                                }

                                Kirigami.Separator {
                                    Layout.fillWidth: true
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        Item {
            Layout.preferredHeight: Kirigami.Units.gridUnit * 2
        }
    }
}
