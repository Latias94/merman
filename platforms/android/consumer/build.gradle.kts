import org.gradle.api.artifacts.component.ModuleComponentIdentifier

plugins {
    alias(libs.plugins.android.application)
}

val selectedGroup = providers.gradleProperty("mermanGroup").get()
val selectedVersion = providers.gradleProperty("mermanVersion").get()

android {
    namespace = "org.example.merman.consumer"
    compileSdk = 35
    defaultConfig {
        applicationId = "org.example.merman.consumer"
        minSdk = 23
        targetSdk = 35
        versionCode = 1
        versionName = "1"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    packaging {
        jniLibs {
            useLegacyPackaging = false
            keepDebugSymbols += "**/libmerman_android_jni.so"
        }
    }
}

dependencies {
    implementation("$selectedGroup:merman-android:$selectedVersion")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
}

tasks.register("verifyMermanResolution") {
    doLast {
        val components = configurations["debugRuntimeClasspath"].incoming.resolutionResult
            .allComponents.mapNotNull { it.id as? ModuleComponentIdentifier }
        check(components.any {
            it.group == selectedGroup && it.module == "merman-android" &&
                it.version == selectedVersion
        }) { "The consumer did not resolve the exact Maven coordinate" }
        check(components.any {
            it.group == "org.jetbrains.kotlin" && it.module == "kotlin-stdlib"
        }) { "The consumer did not resolve the transitive Kotlin runtime" }
        components.sortedBy { it.displayName }.forEach { println(it.displayName) }
    }
}
