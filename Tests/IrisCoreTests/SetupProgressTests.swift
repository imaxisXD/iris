import Foundation
import Testing
@testable import IrisCore

@Suite struct SetupProgressTests {
    typealias Facts = SetupProgress.Facts

    @Test func aNewMacStartsAtAllowWithTheRestWaiting() {
        let p = SetupProgress(Facts(trusted: false, modelOnDisk: false))
        #expect(p.current == .allow)
        #expect(p.status(of: .allow) == .now)
        #expect(p.status(of: .model) == .later)
        #expect(p.status(of: .tryIt) == .later)
        #expect(!p.isComplete)
        #expect(!p.offersSetUpLater)
    }

    /// Steps go in order: a model downloaded before permission doesn't skip step 1, but it
    /// shows as done so nobody is asked to download it again.
    @Test func permissionComesFirstEvenWithTheModelOnDisk() {
        let p = SetupProgress(Facts(trusted: false, modelOnDisk: true))
        #expect(p.current == .allow)
        #expect(p.status(of: .model) == .done)
        #expect(p.status(of: .tryIt) == .later)
    }

    @Test func allowedMovesOnToTheModel() {
        let p = SetupProgress(Facts(trusted: true, modelOnDisk: false, activity: .downloading(0.4)))
        #expect(p.current == .model)
        #expect(p.status(of: .allow) == .done)
        #expect(p.status(of: .model) == .now)
    }

    /// The bug: Finish could be pressed at any step. Now the model on disk and permission
    /// aren't enough; the practice box has to be fixed by Iris once.
    @Test func setupIsNotCompleteUntilIrisFixedThePracticeText() {
        let ready = Facts(trusted: true, modelOnDisk: true, activity: .ready)
        #expect(SetupProgress(ready).current == .tryIt)
        #expect(SetupProgress(ready).preloads == false)  // already in memory
        var tried = ready
        tried.tried = true
        #expect(SetupProgress(tried).isComplete)
        #expect(SetupProgress(tried).status(of: .tryIt) == .done)
    }

    /// The bug: the shortcut waited for setup to be complete, but Try it is pressing the
    /// shortcut, so setup could never finish.
    @Test func theShortcutWorksFromTryItOn() {
        #expect(SetupProgress(Facts(trusted: true, modelOnDisk: true)).current == .tryIt)
        #expect(SetupProgress(Facts(trusted: true, modelOnDisk: true)).canCheck)
        #expect(!SetupProgress(Facts(trusted: true, modelOnDisk: false)).canCheck)
        #expect(!SetupProgress(Facts(trusted: false, modelOnDisk: true)).canCheck)
    }

    @Test func tryItPreloadsAModelThatIsOnlyOnDisk() {
        #expect(SetupProgress(Facts(trusted: true, modelOnDisk: true)).preloads)
        #expect(!SetupProgress(Facts(trusted: true, modelOnDisk: true, activity: .loading)).preloads)
    }

    /// Someone set up before whose permission was lost (an update re-signed Iris): back to
    /// step 1, the rest already done, and not treated as new.
    @Test func lostPermissionAfterSetupIsReturningNotNew() {
        let p = SetupProgress(Facts(trusted: false, modelOnDisk: true, finishedBefore: true))
        #expect(p.current == .allow)
        #expect(p.isReturning)
        #expect(p.status(of: .model) == .done)
        #expect(p.status(of: .tryIt) == .done)
    }

    /// Deleting the model after setup brings back step 2 only.
    @Test func deletedModelAfterSetupNeedsOnlyTheModel() {
        let p = SetupProgress(Facts(trusted: true, modelOnDisk: false, finishedBefore: true))
        #expect(p.current == .model)
        #expect(p.status(of: .tryIt) == .done)
        #expect(!p.isComplete)
    }

    @Test func comingBackFromSettingsUnallowedIsStuckWithAWayOut() {
        let p = SetupProgress(Facts(trusted: false, modelOnDisk: false, backFromSettingsUntrusted: true))
        #expect(p.status(of: .allow) == .stuck)
        #expect(p.offersSetUpLater)
    }

    @Test func aFailedDownloadIsStuck() {
        let p = SetupProgress(Facts(trusted: true, modelOnDisk: false,
                                    activity: .failed(.noRoom(needed: "6.2 GB", free: "3.1 GB"))))
        #expect(p.status(of: .model) == .stuck)
        #expect(p.offersSetUpLater)
    }

    @Test func onlyRetriesThatCanWorkAreOffered() {
        #expect(ModelFailure.offline.canRetry)
        #expect(!ModelFailure.noRoom(needed: "6.2 GB", free: "3.1 GB").canRetry)
        #expect(!ModelFailure.notDownloaded.canRetry)
    }

    @Test func networkErrorsReadAsOfflineEvenWhenWrapped() {
        #expect(ModelFailure.isOffline(URLError(.notConnectedToInternet)))
        let wrapped = NSError(domain: "HubClient", code: 1,
                              userInfo: [NSUnderlyingErrorKey: NSError(domain: NSURLErrorDomain, code: NSURLErrorNetworkConnectionLost)])
        #expect(ModelFailure.isOffline(wrapped))
        #expect(!ModelFailure.isOffline(CocoaError(.fileWriteOutOfSpace)))
    }
}

