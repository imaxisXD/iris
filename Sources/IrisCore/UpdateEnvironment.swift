/// The environment Iris hands to scripts/update.sh. Iris inherits the environment of whatever
/// opened it, a script included, and a build setting left in it (`IRIS_COMMIT`, most of all)
/// would make every Update & reopen rebuild the version already installed. These are the same
/// names as `IRIS_BUILD_VARS` in scripts/lib.sh.
public enum UpdateEnvironment {
    public static let buildVariables: Set<String> = ["IRIS_COMMIT", "IRIS_SOURCE_PATH", "SIGN_IDENTITY"]

    /// `environment` without the build settings, and never stopping to ask for a password.
    public static func cleaned(_ environment: [String: String]) -> [String: String] {
        var env = environment.filter { !buildVariables.contains($0.key) }
        env["GIT_TERMINAL_PROMPT"] = "0"
        return env
    }
}
