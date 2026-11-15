tags:: AI, study
source:: [[Week 2026-11-09]] (deep work 1/3)

- ## Embeddings
	- String into vectors R^d
	- vectors with dimension d
	- each chunk is a vector
	- used for similarity search
- ## Cosine similarity
	- Derivation:
		- scalar product
		  $$
		  \mathbf{a} \cdot \mathbf{b} = \sum_{i}^d \mathbf{a_i} \mathbf{b_i}
		  $$
		- geometric deffinition
		  $$
		  \mathbf{a} \cdot \mathbf{b} = \|\mathbf{a}\| \|\mathbf{b}\| \cos{\theta}
		  $$
		- dividing both by $\|\mathbf{a}\|\|\mathbf{b}\|$
		  $$
		  \cos{\theta} = \frac{\mathbf{a} \cdot \mathbf{b}}{\|\mathbf{a}\|\|\mathbf{b}\|}
		  $$
			- with $\|\mathbf{x}\| = \sqrt{\sum_{i}^{d} \mathbf{x}_{i}^{2}}$
			  $$
			  \cos{\theta} = \frac{\sum_{i}^d \mathbf{a_i} \mathbf{b_i}}{\sqrt{\sum_{i}^{d} \mathbf{a}_{i}^{2}} \sqrt{\sum_{i}^{d} \mathbf{b}_{i}^{2}}}
			  $$
		- the search
			- $\cos{\theta} \in [-1, 1]$
				- 1 = same direction or similar
				- -1 = opposite direction or different
				- 0 = unrelated
		- so, it would cost approximately, for N chunks
			- $N \times 3 d$
		- with normalisation $\|\mathbf{a}\| = \|\mathbf{b}\| = 1$, then
		  $$
		  \cos{\theta} = \mathbf{a} \cdot \mathbf{b}
		  $$
	- Why normalise at ingest:
		- transforms a $3N d$ into $N d$ operations/search at a cost of normalisation once
		- search speed increase
		- trade-off is paid once at ingestion
		-
- ## RAG pipeline
	- Ingest (once)
		- docs -> chunks -> embed -> store
	- Query (every question)
		- question -> chunks -> embed -> top-k by cosine -> prompt
		- prompt + question -> LLM -> answer
	- What goes into the prompt:
		- the k chunks and the question
- ## Chunking and top-k
	- Chunk size trade-off:
		- match
			- too little -> ambiguous
			- too much -> diluted
		- context
			- too little -> disconection
			- to much -> tends to a limit
		- sweet spot between 200 and 800 tokens
	- top-k:
		- how many chunks goes into the prompt
		- too few -> increase a retrieval miss chance
		- too many -> increase cost and noise
- ## How RAG fails
	- retrieval miss
		- the search didn't retrieve the existing desired chunk
	- chunk boundary
		- the answer is split across two chunks
	- hallucination
		- the right context comes back but the model still answers beyond it
	- stale index
		- the docs changed but were not re-embeded
	-
- ## Agents
	- The loop:
		- question
		- LLM
		- if LLM reply is a tool call -> call it and loop back to LLM with LLM tool call and its return
		- if LLM reply is the final answer -> return it
	- LLM job vs my program's job:
		- LLM will interpret the operation steps
		- my program is going to coordinate its steps and side effects
	- Why tools:
		- because we need to be responsible for our operations
			- validations
			- human confirmations
			- idempotency
			- security: prompt injection
- ## How agents fail
	- infinite loop
		- leads to max attempts limit
	- bad arguments
		- leads to validation
	- duplicate side effects
		- leads to idempotency
	- hanging tool
		- leads to timeout plus cancellation
	- prompt injection
		- treat tool output as data
		- limit what tools can do
- ## MCP
	- Model Context Protocol
	- protocol between the HOST (my program) and tool SERVERS, not the model
		- tools/list = discovery -> becomes the tool menu in the prompt
		- tools/call = execution on the server
		- why a standard: N apps x M tools of glue -> N + M
		- caipora sits in the middle
- ## Evaluation
	- Retrieval vs answer:
		- if answer is wrong and the right-chunk was NOT in top-k, it is a retrieval problem
		- if answer is wrong and the right-chunk WAS in top-key, it is a model problem
			- the model ignored or misread the context
			- fix the prompt
		- if answer is correct and the right-chunk was NOT in top-key, it is luck or the model already knew it; it shouldn't be trusted
- ## When NOT to use RAG or an agent
	- RAG
		- docs fit in the prompt
			- a retrieval step can miss
		- exact lookups
			- that's an info query, not similarity search
	- AGENTS
		- steps are always the same
		- costs only payoff when the next step depends on what came back
			-
- ## Links to what I already know
	- Little's law, back pressure, idempotency, cancellation (#1325), MCP gateway:
- ## What I missed (after review)
	- review:: 2026-11-10, with Claude
	- Query: the question is NOT chunked. question -> embed -> top-k
		- Embed the question with the SAME model as the chunks (each model has its own vector space)
		- Normalise the question vector too
	- The store keeps vector + original text per chunk. Vectors only FIND chunks; the TEXT goes into the prompt. The LLM never sees a vector
	- Prompt also has the instruction: "answer only from the context; if it isn't there, say you don't know" (reduces hallucination, doesn't remove it)
	- Embeddings: the key property is that similar meaning lands close together. Bigger d = more nuance, more memory and compute
	- top-k = how many CHUNKS go into the prompt (not answers)
	- Chunking: add a small overlap so a sentence isn't cut at the boundary
	- Agent loop: the stop condition MAX_STEPS belongs in the loop itself
	- LLM's job, more precisely: choose the next tool + args, read results, decide when it's done, write the answer. It only produces text; MY PROGRAM runs the tool
	- Why tools at all: an LLM can't read a DB, open a file or send an email. Tools give it eyes and hands; my program holds the keys (validation, confirmation, idempotency, injection = why MY CODE runs them)
	- Evaluation: recall@k = hits / questions; change ONE thing (chunk size, k, prompt), re-run, compare
	- Exact lookup = SQL query. Free-form data questions: an LLM writes the SQL as a tool (text-to-SQL), the answer still comes from the DB
	- Missing: where I've already seen an agent loop -> Claude Code (read file -> cargo test -> edit) and my MCP gateway
	- Links section still empty: Little's law for slow LLM calls, 429 = reject at the edge + backoff/jitter, back pressure on embedding ingest, idempotency for side-effect tools, select! timeout for hanging tools