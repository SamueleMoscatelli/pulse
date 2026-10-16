# Architecture

The service is split into an API layer, a worker pool and a scheduler. Requests enter
through the API layer, which validates input and enqueues jobs. Workers pull jobs from
the queue, run them with a timeout and write results to the store. The scheduler
re-enqueues failed jobs with exponential backoff and alerts after five failures.
