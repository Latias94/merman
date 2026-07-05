plugins {
    id("com.android.library") version "9.2.0"
    id("maven-publish")
    id("signing")
}

group = "io.merman"
version = "0.8.0-alpha.2"

android {
    namespace = "io.merman"
    compileSdk = 35

    defaultConfig {
        minSdk = 23
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("consumer-rules.pro")

        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    publishing {
        singleVariant("release") {
            withSourcesJar()
            withJavadocJar()
        }
    }

    sourceSets {
        getByName("androidTest") {
            java.srcDir("examples")
        }
    }
}

dependencies {
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
}

publishing {
    publications {
        create<MavenPublication>("release") {
            groupId = "io.merman"
            artifactId = "merman-android"
            version = project.version.toString()

            pom {
                name.set("merman-android")
                description.set("Android JNI bindings for merman headless Mermaid rendering.")
                url.set("https://github.com/Latias94/merman")

                licenses {
                    license {
                        name.set("MIT License")
                        url.set("https://opensource.org/license/mit")
                        distribution.set("repo")
                    }
                    license {
                        name.set("Apache License, Version 2.0")
                        url.set("https://www.apache.org/licenses/LICENSE-2.0")
                        distribution.set("repo")
                    }
                }

                developers {
                    developer {
                        id.set("frankorz")
                        name.set("Mingzhen Zhuang")
                        email.set("superfrankie621@gmail.com")
                    }
                }

                scm {
                    connection.set("scm:git:https://github.com/Latias94/merman.git")
                    developerConnection.set("scm:git:ssh://git@github.com/Latias94/merman.git")
                    url.set("https://github.com/Latias94/merman")
                }
            }
        }
    }

    repositories {
        maven {
            name = "localStaging"
            url = layout.buildDirectory.dir("repo").get().asFile.toURI()
        }
    }
}

afterEvaluate {
    publishing {
        publications.named<MavenPublication>("release") {
            from(components["release"])
        }
    }
}

signing {
    val signingKey = providers.gradleProperty("signingInMemoryKey")
        .orElse(providers.environmentVariable("ORG_GRADLE_PROJECT_signingInMemoryKey"))
    val signingPassword = providers.gradleProperty("signingInMemoryKeyPassword")
        .orElse(providers.environmentVariable("ORG_GRADLE_PROJECT_signingInMemoryKeyPassword"))

    if (signingKey.isPresent) {
        useInMemoryPgpKeys(signingKey.get(), signingPassword.orNull)
        sign(publishing.publications["release"])
    }
}
