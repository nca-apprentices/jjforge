package dev.nca.jjforge

import org.awaitility.Awaitility.await
import org.junit.jupiter.api.Test
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.context.ApplicationEventPublisher
import org.springframework.context.annotation.Import
import org.springframework.jdbc.core.simple.JdbcClient
import org.springframework.modulith.events.ApplicationModuleListener
import org.springframework.modulith.events.IncompleteEventPublications
import org.springframework.stereotype.Component
import org.springframework.transaction.support.TransactionTemplate
import java.util.UUID
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.test.assertEquals

/**
 * The event publication registry keeps a publication in Postgres until its
 * listener completes, as ADR 0002 decides. A failed listener leaves the row,
 * and a resubmission that succeeds removes it.
 */
@SpringBootTest
@Import(TestcontainersConfiguration::class, EventPublicationTest.Listener::class)
class EventPublicationTest(
    @Autowired private val events: ApplicationEventPublisher,
    @Autowired private val transactions: TransactionTemplate,
    @Autowired private val incomplete: IncompleteEventPublications,
    @Autowired private val jdbc: JdbcClient,
    @Autowired private val listener: Listener,
) {
    data class Pinged(
        val id: UUID,
    )

    @Component
    class Listener {
        @Volatile
        var failing = true
        val received = CopyOnWriteArrayList<Pinged>()

        @ApplicationModuleListener
        fun on(event: Pinged) {
            check(!failing) { "listener is failing" }
            received.add(event)
        }
    }

    private fun publications() = jdbc.sql("SELECT count(*) FROM event_publication").query(Long::class.java).single()

    @Test
    fun `keeps a publication until its listener completes`() {
        val event = Pinged(UUID.randomUUID())

        transactions.executeWithoutResult { events.publishEvent(event) }
        await().until {
            jdbc
                .sql("SELECT status FROM event_publication")
                .query(String::class.java)
                .optional()
                .orElse(null) == "FAILED"
        }

        listener.failing = false
        incomplete.resubmitIncompletePublications { true }
        await().until { publications() == 0L }

        assertEquals(listOf(event), listener.received)
    }
}
