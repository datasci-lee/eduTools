import CoreGraphics
import Darwin
import Foundation

public func hasWindowServerConnection() -> Bool {
  CGSessionCopyCurrentDictionary() != nil && CGMainDisplayID() != 0
}

public func requireWindowServer() {
  guard hasWindowServerConnection() else {
    FileHandle.standardError.write(Data("UNRUNNABLE: WindowServer 연결 없음\n".utf8))
    exit(3)
  }
}

#if WINDOW_SERVER_CHECK_MAIN
@main
private enum WindowServerCheck {
  static func main() {
    requireWindowServer()
  }
}
#endif
