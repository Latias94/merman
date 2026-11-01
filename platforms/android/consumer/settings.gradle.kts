pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        exclusiveContent {
            forRepository {
                maven {
                    name = "selectedMermanPublication"
                    url = uri(providers.gradleProperty("mermanRepository").get())
                    metadataSources {
                        if (providers.gradleProperty("mermanMetadata").orElse("gradle").get() == "pom") {
                            mavenPom()
                            ignoreGradleMetadataRedirection()
                        } else {
                            gradleMetadata()
                        }
                    }
                }
            }
            filter {
                includeModule(providers.gradleProperty("mermanGroup").get(), "merman-android")
            }
        }
        google()
        mavenCentral()
    }
}

rootProject.name = "merman-maven-consumer"
