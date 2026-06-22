import org.gradle.api.GradleException
import org.jetbrains.intellij.platform.gradle.TestFrameworkType

plugins {
    id("java")
    id("org.jetbrains.kotlin.jvm") version "2.0.21"
    id("org.jetbrains.intellij.platform") version "2.2.1"
}

group = "com.aomr"
version = providers.gradleProperty("pluginVersion").get()

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

    testImplementation("junit:junit:4.13.2")
    testImplementation("com.google.code.gson:gson:2.11.0")
}

kotlin {
    jvmToolchain(21)
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

    val validateBundledResources by registering {
        group = "verification"
        description = "Fails the build if required bundled resources are missing."
        doLast {
            val resourcesDir = layout.projectDirectory.dir("src/main/resources").asFile
            val required = listOf(
                "syscalls.json",
                "aiplans.json",
                "syntaxes/xs.tmLanguage.json",
            )
            val missing = required.filter { !resourcesDir.resolve(it).exists() }
            if (missing.isNotEmpty()) {
                throw GradleException(
                    "Missing required bundled resources: ${missing.joinToString()}. " +
                        "See vendor strategy in openspec/changes/intellij-xs-plugin/design.md."
                )
            }
        }
    }

    buildPlugin {
        dependsOn(validateBundledResources)
    }

    test {
        dependsOn(validateBundledResources)
    }
}
