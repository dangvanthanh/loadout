import Foundation
import UserNotifications

/// macOS notification when new skill updates show up.
enum Notifier {
    static func requestPermission() {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    static func postUpdates(_ count: Int) {
        let content = UNMutableNotificationContent()
        content.title = "Skill updates available"
        content.body = "\(count) skill\(count == 1 ? "" : "s") can be updated. Open SkillDock to update."
        let request = UNNotificationRequest(identifier: "skilldock.updates", content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }
}
