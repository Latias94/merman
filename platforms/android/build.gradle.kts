plugins {
    id("com.android.library") version "9.2.0"
    id("org.jetbrains.kotlin.android") version "1.9.24"
}

group = "io.merman"
version = "0.7.0"

android {
    namespace = "io.merman"
    compileSdk = 35

    defaultConfig {
        minSdk = 23
        consumerProguardFiles("consumer-rules.pro")

        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}
