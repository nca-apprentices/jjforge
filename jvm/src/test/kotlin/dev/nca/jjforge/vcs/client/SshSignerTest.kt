package dev.nca.jjforge.vcs.client

import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path

/** vcs checks the server's signature as `ssh-keygen -Y` does, so `ssh-keygen` checks it here. */
class SshSignerTest {
    @TempDir
    lateinit var dir: Path

    private fun run(vararg command: String): Process =
        ProcessBuilder(*command).directory(dir.toFile()).redirectErrorStream(true).start().also { it.waitFor() }

    @Test
    fun `ssh-keygen accepts the signature`() {
        run("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "server", "-f", "id")
        Files.write(dir.resolve("nonce"), byteArrayOf(1, 2, 3))

        val signature = SshSigner.read(dir.resolve("id")).sign(byteArrayOf(1, 2, 3), "jjforge-session")
        Files.writeString(dir.resolve("nonce.sig"), signature)
        val principal = "server ${Files.readString(dir.resolve("id.pub")).trim()}"
        Files.writeString(dir.resolve("allowed"), principal)
        val check =
            ProcessBuilder(
                "ssh-keygen", "-Y", "verify", "-f", "allowed", "-I", "server",
                "-n", "jjforge-session", "-s", "nonce.sig",
            ).directory(dir.toFile())
                .redirectInput(dir.resolve("nonce").toFile())
                .redirectErrorStream(true)
                .start()

        assertThat(check.waitFor()).describedAs(check.inputStream.readAllBytes().decodeToString()).isZero()
    }

    @Test
    fun `wraps the armor at 70 columns, as ssh-keygen does`() {
        run("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", "id")

        val lines = SshSigner.read(dir.resolve("id")).sign(ByteArray(56), "jjforge-session").lines()

        val body = lines.drop(1).takeWhile { !it.startsWith("-----") }
        assertThat(body.dropLast(1)).allSatisfy { assertThat(it).hasSize(70) }
    }

    @Test
    fun `refuses an encrypted key`() {
        run("ssh-keygen", "-q", "-t", "ed25519", "-N", "secret", "-f", "id")

        assertThat(runCatching { SshSigner.read(dir.resolve("id")) }.exceptionOrNull())
            .hasMessageContaining("encrypted")
    }
}
