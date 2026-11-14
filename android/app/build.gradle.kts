plugins {
    id("com.android.application")
}

android {
    namespace = "com.voltix.monitor"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.voltix.monitor"
        minSdk = 26
        targetSdk = 35
        versionCode = 4
        versionName = "0.1.0-alpha.3"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    val ksPath = System.getenv("VOLTIX_KEYSTORE_PATH")
    val ksPass = System.getenv("VOLTIX_KEYSTORE_PASSWORD")
    val ksAlias = System.getenv("VOLTIX_KEY_ALIAS")
    val keyPass = System.getenv("VOLTIX_KEY_PASSWORD")
    val hasReleaseSigning = !ksPath.isNullOrBlank() &&
        !ksPass.isNullOrBlank() &&
        !ksAlias.isNullOrBlank() &&
        !keyPass.isNullOrBlank() &&
        file(ksPath).exists()

    gradle.taskGraph.whenReady {
        if (allTasks.any { it.name.contains("release", ignoreCase = true) }) {
            require(hasReleaseSigning) {
                "Release APKs require the existing Voltix signing key: set all VOLTIX_KEYSTORE_* and VOLTIX_KEY_* variables."
            }
        }
    }

    signingConfigs {
        if (hasReleaseSigning) {
            create("release") {
                storeFile = file(ksPath)
                storePassword = ksPass
                keyAlias = ksAlias
                keyPassword = keyPass
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (hasReleaseSigning) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
        debug {
            isDebuggable = true
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
