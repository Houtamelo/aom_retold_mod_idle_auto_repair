import org.gradle.api.GradleException
import org.jetbrains.intellij.platform.gradle.TestFrameworkType

plugins {
    id("java")
    id("org.jetbrains.kotlin.jvm") version "2.0.21"
    id("org.jetbrains.intellij.platform") version "2.2.1"
    id("org.jetbrains.grammarkit") version "2022.3.2"
}

group = "com.aomr"
version = providers.gradleProperty("pluginVersion").get()

// `buildDir` is normally `tools/intellij-xs-plugin/build/`, but a previous
// build under a different uid (100999) left files there that this user
// cannot modify. Redirect to the repo-root `dist/.build/` — a sibling of
// `dist/` that holds intermediate build artifacts. The whole `dist/` tree
// is gitignored, so artifacts inside `dist/.build/` are never committed.
// `copyDistributionToDist` (defined further down) moves the final .zip from
// `dist/.build/distributions/` up to `dist/` so it sits alongside the
// published artifacts (see docs/smoke-test-setup.md for the convention).
project.buildDir = file("../../dist/.build")

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        create("IC", providers.gradleProperty("platformVersion"))
        bundledPlugins(
            "org.jetbrains.plugins.textmate",
        )
        testFramework(TestFrameworkType.Platform)
    }

    implementation("org.eclipse.lsp4j:org.eclipse.lsp4j:0.24.0")

    testImplementation("junit:junit:4.13.2")
    testImplementation("com.google.code.gson:gson:2.11.0")
}

kotlin {
    jvmToolchain(21)
}

sourceSets {
    main {
        java {
            srcDir("src/main/gen")
        }
    }
}

tasks {
    withType<JavaCompile> {
        sourceCompatibility = "21"
        targetCompatibility = "21"
    }

    patchPluginXml {
        sinceBuild.set(providers.gradleProperty("pluginSinceBuild"))
        untilBuild.set(providers.gradleProperty("pluginUntilBuild"))
    }

    generateLexer {
        sourceFile.set(file("src/main/kotlin/com/aomr/xs/psi/XsLexer.flex"))
        targetDir.set("src/main/gen/com/aomr/xs/psi")
        targetClass.set("XsLexer")
        purgeOldFiles.set(true)
    }

    compileJava {
        dependsOn(generateLexer)
    }

    compileKotlin {
        dependsOn(generateLexer)
    }

    val validateBundledResources by registering {
        group = "verification"
        description = "Fails the build if required bundled resources are missing."
        doLast {
            val resourcesDir = layout.projectDirectory.dir("src/main/resources").asFile
            val required = listOf(
                "syntaxes/xs.tmLanguage.json",
                "bin/xs-language-server",
            )
            val missing = required.filter { !resourcesDir.resolve(it).exists() }
            if (missing.isNotEmpty()) {
                throw GradleException(
                    "Missing required bundled resources: ${missing.joinToString()}. " +
                        "Run `./gradlew stageLspServer` (or let `buildPlugin` do it) " +
                        "and retry. See docs/smoke-test-setup.md."
                )
            }
        }
    }

    // Build the Rust LSP server and copy the release binary into the
    // plugin's bundled resources. This is what makes the plugin zip
    // self-contained: `XsLspConnection` extracts the binary from the
    // classpath at runtime and runs it as a stdio child process.
    //
    // The IntelliJ plugin's gradle project is standalone (no top-level
    // settings.gradle), so `rootProject` is the plugin itself. The LSP
    // crate lives at `tools/xs-language-server` — a sibling of this
    // plugin's directory. We resolve relative to `rootDir.parentFile`.
    val buildLspServer by registering {
        group = "build"
        description = "Build the XS Language Server Rust crate (cargo build --release)."
        doLast {
            val lspCrate = rootDir.parentFile.resolve("xs-language-server")
            if (!lspCrate.exists()) {
                throw GradleException(
                    "LSP crate not found at $lspCrate. " +
                        "Expected alongside the IntelliJ plugin under tools/."
                )
            }
            exec {
                workingDir = lspCrate
                commandLine("cargo", "build", "--release")
            }
        }
    }

    val stageLspServer by registering(Copy::class) {
        group = "build"
        description = "Stage the LSP release binary into src/main/resources/bin/."
        from(rootDir.parentFile.resolve("xs-language-server/target/release/xs-language-server"))
        into(layout.projectDirectory.dir("src/main/resources/bin"))
        dependsOn(buildLspServer)
        // The destination is .gitignored (see repo root .gitignore); we
        // always overwrite on each build so the bundled binary matches the
        // source.
        outputs.upToDateWhen { false }
    }

    // processResources must declare the dependency explicitly so the
    // resources/bin/xs-language-server file is in place when the JAR is
    // packaged. Otherwise gradle reports an implicit-dependency error.
    processResources {
        dependsOn(stageLspServer)
    }

    buildPlugin {
        dependsOn(validateBundledResources)
        dependsOn(stageLspServer)
    }

    // Move the final .zip from buildDir/distributions/ up to the repo-root
    // dist/ folder so it sits alongside the documented smoke-test artifacts
    // (see docs/smoke-test-setup.md). Runs after `buildPlugin` regardless of
    // success because we want the .zip on disk even if downstream tasks fail.
    // The plugin project lives at tools/intellij-xs-plugin/, so the repo
    // root is `rootDir.parentFile.parentFile` — `../../dist` from here.
    val copyDistributionToDist by registering(Copy::class) {
        group = "build"
        description = "Copy the plugin .zip from build/distributions/ to the repo-root dist/ folder."
        from(layout.buildDirectory.dir("distributions"))
        into(rootDir.parentFile.parentFile.resolve("dist"))
        include("*.zip")
    }

    buildPlugin {
        finalizedBy(copyDistributionToDist)
    }

    test {
        dependsOn(validateBundledResources)
        // Each fixture test class uses its own IDE sandbox; forking avoids
        // a platform lifecycle hang when multiple BasePlatformTestCase classes
        // run in the same JVM in this headless environment.
        forkEvery = 1
    }
}
