extends Control

func _ready():
	test_group_signatures()

func test_group_signatures():
	var nostringer = Nostringer.new()
	print("--- Starting Group (Ring) Signature Test ---")

	var private_keys = []
	var public_keys = []
	for i in range(5):
		var kp = nostringer.generate_keypair("xonly")
		private_keys.append(kp["private_key"])
		public_keys.append(kp["public_key"])
	
	var message = "Vote: Proposal #123".to_utf8_buffer()
	var votes_db = {} 

	var vote = func(voter_name: String, priv_key: String):
		# AÑADIDO 5º ARGUMENTO ""
		var sign_res = nostringer.sign(message, priv_key, public_keys, "blsag")
		var sig = sign_res["signature"]
		
		print("\n[%s] is voting..." % voter_name)
		var verify_res = nostringer.verify(sig, message, public_keys)
		
		if not verify_res.get("valid", false):
			print("  ERROR: Invalid signature!")
			return

		var ki = verify_res.get("key_image", "")
		if votes_db.has(ki):
			print("  REJECTED: Double vote detected!")
		else:
			votes_db[ki] = voter_name
			print("  SUCCESS: Vote counted.")

	vote.call("Voter Alpha", private_keys[0])
	vote.call("Voter Beta", private_keys[2])
	vote.call("Voter Alpha (Attempt 2)", private_keys[0])

	print("\n--- Summary ---")
	print("Total physical votes in group: ", votes_db.size())
	print("--- Group Test Completed ---")
