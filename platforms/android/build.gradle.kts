import org.gradle.api.attributes.Bundling
import org.gradle.api.attributes.Category
import org.gradle.api.attributes.DocsType
import org.gradle.api.attributes.Usage
import org.gradle.api.component.AdhocComponentWithVariants

plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.dokka)
    id("maven-publish")
    id("signing")
}

group = "io.merman"
version = "0.8.0-alpha.6"

android {
    namespace = "io.merman"
    compileSdk = 35
    ndkVersion = libs.versions.ndk.get()

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
        }
    }

    sourceSets {
        getByName("androidTest") {
            kotlin.directories += "examples"
        }
    }

    packaging {
        resources {
            // AGP excludes this standard path by default, but release artifacts must carry it.
            excludes -= "/META-INF/LICENSE"
        }
    }
}

dokka {
    dokkaPublications.html {
        moduleName.set("merman-android")
        moduleVersion.set(project.version.toString())
        offlineMode.set(true)
    }
}

val dokkaHtmlJar = tasks.register<org.gradle.jvm.tasks.Jar>("dokkaHtmlJar") {
    archiveClassifier.set("javadoc")
    from(tasks.named("dokkaGeneratePublicationHtml"))
}

val dokkaHtmlElements = configurations.create("dokkaHtmlElements") {
    isCanBeConsumed = true
    isCanBeResolved = false
    attributes {
        attribute(Category.CATEGORY_ATTRIBUTE, objects.named(Category.DOCUMENTATION))
        attribute(Bundling.BUNDLING_ATTRIBUTE, objects.named(Bundling.EXTERNAL))
        attribute(DocsType.DOCS_TYPE_ATTRIBUTE, objects.named(DocsType.JAVADOC))
        attribute(Usage.USAGE_ATTRIBUTE, objects.named(Usage.JAVA_RUNTIME))
    }
    outgoing.artifact(dokkaHtmlJar)
}

dependencies {
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
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
    (components["release"] as AdhocComponentWithVariants).addVariantsFromConfiguration(
        dokkaHtmlElements,
    ) {}
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
