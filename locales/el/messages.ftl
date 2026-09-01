# Πόροι τοπικοποίησης για τη γραμμή εντολών του Netsuke.

runner.io.dyndep.retention = Δεν ήταν δυνατή η εφαρμογή της διατήρησης του παραγόμενου dyndep κάτω από τη διαδρομή { $path }.
cli.about = Το Netsuke μεταγλωττίζει δηλωτικά YAML + Jinja σε σχέδια δόμησης Ninja.
cli.long_about = Το Netsuke μετατρέπει δηλωτικά YAML + Jinja σε αναπαραγώγιμα γραφήματα Ninja και εκτελεί το Ninja με ασφαλείς προεπιλογές.
cli.usage = { $usage }

# Κείμενο βοήθειας για τις γενικές επιλογές.
cli.flag.file.help = Διαδρομή προς το αρχείο δηλωτικού του Netsuke που θα χρησιμοποιηθεί.
cli.flag.directory.help = Εκτέλεση σαν να είχε ξεκινήσει σε αυτόν τον κατάλογο.
cli.flag.config.help = Διαδρομή προς αρχείο ρυθμίσεων, παρακάμπτοντας την αυτόματη αναζήτηση.
cli.flag.jobs.help = Ορισμός του πλήθους των παράλληλων εργασιών δόμησης.
cli.flag.verbose.help = Ενεργοποίηση αναλυτικής διαγνωστικής καταγραφής και συνόψεων χρόνου στο τέλος.
cli.flag.locale.help = Ετικέτα γλώσσας για τα κείμενα της γραμμής εντολών (για παράδειγμα: en-US, el).
cli.flag.fetch_allow_scheme.help = Πρόσθετα σχήματα URL που επιτρέπονται στο βοήθημα fetch.
cli.flag.fetch_allow_host.help = Ονόματα κόμβων που επιτρέπονται όταν ισχύει η προεπιλεγμένη άρνηση.
cli.flag.fetch_block_host.help = Ονόματα κόμβων που αποκλείονται πάντοτε, ακόμη κι αν επιτρέπονται αλλού.
cli.flag.fetch_default_deny.help = Άρνηση όλων των κόμβων από προεπιλογή· να επιτρέπεται μόνο ο δηλωμένος κατάλογος.
cli.flag.json.help = Παραγωγή εξόδου JSON αναγνώσιμης από μηχανή.
cli.flag.no_input.help = Να μη γίνεται ποτέ ανάγνωση διαδραστικής εισόδου.
cli.flag.color.help = Πολιτική έγχρωμης εξόδου (auto, always, never).
cli.flag.emoji.help = Πολιτική για τα emoji (auto, always, never).
cli.flag.progress.help = Πολιτική εμφάνισης της προόδου (auto, always, never).
cli.flag.accessibility.help = Πολιτική προσβάσιμης εξόδου (auto, on, off).
cli.flag.default_targets.help = Προεπιλεγμένοι στόχοι δόμησης όταν δεν ορίζεται κανένας.

# Περιγραφές υποεντολών.
cli.subcommand.build.about = Δόμηση των στόχων που ορίζονται στο δηλωτικό (προεπιλογή).
cli.subcommand.build.long_about = Δόμηση των ζητούμενων στόχων· αν δεν δοθεί κανένας, χρήση των προεπιλεγμένων στόχων του δηλωτικού.
cli.subcommand.clean.about = Αφαίρεση των τεχνουργημάτων δόμησης μέσω του Ninja.
cli.subcommand.clean.long_about = Δημιουργία προσωρινού αρχείου Ninja και έπειτα εκτέλεση του `ninja -t clean`.
cli.subcommand.graph.about = Εξαγωγή του γραφήματος εξαρτήσεων δόμησης. Η προεπιλεγμένη μορφή είναι DOT.
cli.subcommand.graph.long_about = Προβολή του αναλυμένου δηλωτικού Netsuke σε κανονικό γράφημα δόμησης και εγγραφή του ως Graphviz DOT ή, με την επιλογή `--html`, ως αυτοτελής σελίδα HTML. Χρησιμοποιήστε `--output <ΑΡΧΕΙΟ>` για εγγραφή σε αρχείο· το `-` γράφει στην τυπική έξοδο.
cli.subcommand.generate.about = Δημιουργία του δηλωτικού Ninja χωρίς εκτέλεση του Ninja.
cli.subcommand.generate.long_about = Εγγραφή του παραγόμενου δηλωτικού Ninja στην τυπική έξοδο ή σε αρχείο που επιλέγεται με `--output`.
cli.subcommand.help.about = Εκτυπώστε τη βοήθεια ανώτατου επιπέδου ή τη βοήθεια για ένα ονομασμένο θέμα.
cli.subcommand.help.long_about = Χωρίς θέμα, αυτό ταιριάζει με το `--help`. Χρησιμοποιήστε το `help targets` για να εκτυπώσετε τον κατάλογο στόχων και ενεργειών για το επιλεγμένο αρχείο.

# Help catalogue headings and markers.
cli.help.actions_heading = Ενέργειες:
cli.help.targets_heading = Στόχοι:
cli.help.targets.about = Παράθεση στόχων και ενεργειών στο επιλεγμένο δηλωτικό.
cli.help.default_marker = προεπιλογή
cli.help.conditional_marker = υπό όρους

# Κείμενο βοήθειας για τις επιλογές της υποεντολής build.
cli.subcommand.build.flag.targets.help = Στόχοι προς δόμηση (αν παραλειφθούν, χρησιμοποιούνται οι προεπιλογές του δηλωτικού).

# Κείμενο βοήθειας για τις επιλογές της υποεντολής graph.
cli.subcommand.graph.flag.html.help = Απόδοση του γραφήματος ως αυτοτελούς σελίδας HTML αντί για μορφή DOT.
cli.subcommand.graph.flag.output.help = Εγγραφή του τεχνουργήματος γραφήματος στο ΑΡΧΕΙΟ· χρησιμοποιήστε `-` για την τυπική έξοδο.

# Κείμενο βοήθειας για τις επιλογές της υποεντολής generate.
cli.subcommand.generate.flag.output.help = Εγγραφή του παραγόμενου δηλωτικού Ninja στο ΑΡΧΕΙΟ αντί για την τυπική έξοδο.

# Σφάλματα ελέγχου στη γραμμή εντολών.
cli.validation.jobs.invalid_number = Το { $value } δεν είναι έγκυρος αριθμός.
cli.validation.jobs.out_of_range = Το πλήθος των εργασιών πρέπει να βρίσκεται μεταξύ { $min } και { $max }.
cli.validation.scheme.empty = Το σχήμα δεν πρέπει να είναι κενό.
cli.validation.scheme.invalid_start = Το σχήμα «{ $scheme }» πρέπει να ξεκινά με γράμμα ASCII.
cli.validation.scheme.invalid = Μη έγκυρο σχήμα «{ $scheme }».
cli.validation.locale.empty = Η ετικέτα γλώσσας δεν πρέπει να είναι κενή.
cli.validation.locale.invalid = Μη έγκυρη ετικέτα γλώσσας «{ $locale }».
cli.validation.color.invalid = Μη έγκυρη πολιτική χρώματος «{ $value }». Έγκυρες επιλογές: auto, always, never.
cli.validation.emoji.invalid = Μη έγκυρη πολιτική emoji «{ $value }». Έγκυρες επιλογές: auto, always, never.
cli.validation.progress.invalid = Μη έγκυρη πολιτική προόδου «{ $value }». Έγκυρες επιλογές: auto, always, never.
cli.validation.accessibility.invalid = Μη έγκυρη πολιτική προσβασιμότητας «{ $value }». Έγκυρες επιλογές: auto, on, off.
cli.validation.config.expected_object = Οι τιμές της γραμμής εντολών έπρεπε να σειριοποιηθούν σε αντικείμενο· ελήφθη { $value }.

# Μηνύματα σφάλματος του Clap.
clap-error-missing-argument = Λείπει υποχρεωτικό όρισμα: { $argument }
clap-error-missing-subcommand = Λείπει υποεντολή. Διαθέσιμες επιλογές: { $valid_subcommands }
clap-error-unknown-argument = Άγνωστο όρισμα: { $argument }
clap-error-invalid-value = Μη έγκυρη τιμή για το { $argument }: { $value }
clap-error-invalid-subcommand = Άγνωστη υποεντολή: { $subcommand }
# Σημείωση: το value-validation διατυπώνεται διαφορετικά από το invalid-value
# ώστε να ξεχωρίζουν τα σφάλματα ιδιαίτερων ελεγκτών
# (ErrorKind::ValueValidation) από τις ασυμφωνίες τύπων
# (ErrorKind::InvalidValue).
clap-error-value-validation = Ο έλεγχος απέτυχε για το { $argument }: { $value }

# Σφάλματα και συμφραζόμενα της εκτέλεσης.
runner.manifest.not_found = Το δηλωτικό «{ $manifest_name }» δεν βρέθηκε στον κατάλογο { $directory }.
runner.manifest.not_found.help = Βεβαιωθείτε ότι το δηλωτικό υπάρχει ή δώστε `--file` με τη σωστή διαδρομή.
runner.manifest.path_missing_name = Η διαδρομή δηλωτικού «{ $path }» δεν έχει όνομα αρχείου.
cli.file.non_utf8 = Η διαδρομή δηλωτικού «{ $path }» δεν είναι έγκυρο UTF-8.
runner.manifest.directory_label = κατάλογος `{ $directory }`
runner.manifest.current_directory_label = ο τρέχων κατάλογος
runner.manifest.default_not_declared = Η προεπιλογή του δηλωτικού '{ $default }' δεν ονομάζει δηλωμένη ενέργεια ή στόχο.
runner.context.network_policy = Δεν ήταν δυνατή η κατασκευή της πολιτικής δικτύου.
runner.context.load_manifest = Δεν ήταν δυνατή η φόρτωση του δηλωτικού από { $path }.
runner.context.serialise_manifest = Δεν ήταν δυνατή η σειριοποίηση του δηλωτικού.
runner.context.build_graph = Δεν ήταν δυνατή η κατασκευή γραφήματος από το δηλωτικό.
runner.context.generate_ninja = Δεν ήταν δυνατή η δημιουργία του δηλωτικού Ninja.
runner.context.render_graph = Δεν ήταν δυνατή η απόδοση του τεχνουργήματος γραφήματος.

runner.io.create_temp_file = Δεν ήταν δυνατή η δημιουργία του προσωρινού αρχείου Ninja.
runner.io.write_temp_ninja = Δεν ήταν δυνατή η εγγραφή του προσωρινού αρχείου Ninja.
runner.io.flush_temp_ninja = Δεν ήταν δυνατή η εκκένωση της ενδιάμεσης μνήμης του προσωρινού αρχείου Ninja.
runner.io.sync_temp_ninja = Δεν ήταν δυνατός ο συγχρονισμός του προσωρινού αρχείου Ninja.
runner.io.create_parent_dir = Δεν ήταν δυνατή η δημιουργία του γονικού καταλόγου { $path }.
runner.io.create_ninja_file = Δεν ήταν δυνατή η δημιουργία του αρχείου Ninja στο { $path }.
runner.io.write_ninja_file = Δεν ήταν δυνατή η εγγραφή του αρχείου Ninja στο { $path }.
runner.io.flush_ninja_file = Δεν ήταν δυνατή η εκκένωση της ενδιάμεσης μνήμης του αρχείου Ninja στο { $path }.
runner.io.sync_ninja_file = Δεν ήταν δυνατός ο συγχρονισμός του αρχείου Ninja στο { $path }.
runner.io.open_ambient_dir = Δεν ήταν δυνατό το άνοιγμα του περιβάλλοντος καταλόγου.
cli.directory.non_utf8 = Η διαδρομή του καταλόγου εργασίας δεν είναι έγκυρη UTF-8. ({ $path })
runner.io.no_existing_ancestor = Δεν υπάρχει γονικός κατάλογος για το { $path }.
runner.io.derive_relative_path = Δεν ήταν δυνατή η εξαγωγή της σχετικής διαδρομής Ninja.
runner.io.non_utf8_path = Οι διαδρομές που δεν είναι UTF-8 δεν υποστηρίζονται (διαδρομή: { $path }).
runner.io.write_stdout = Δεν ήταν δυνατή η εγγραφή του δηλωτικού Ninja στην τυπική έξοδο.
runner.io.flush_stdout = Δεν ήταν δυνατή η εκκένωση της τυπικής εξόδου.
runner.io.dyndep.create_dir = Δεν ήταν δυνατή η δημιουργία του καταλόγου dyndep στη διαδρομή { $path }.
runner.io.dyndep.read = Δεν ήταν δυνατή η ανάγνωση του παραγόμενου αρχείου dyndep στη διαδρομή { $path }.
runner.io.dyndep.write = Δεν ήταν δυνατή η εγγραφή του παραγόμενου αρχείου dyndep στη διαδρομή { $path }.
runner.io.dyndep.rename = Δεν ήταν δυνατή η οριστικοποίηση του παραγόμενου αρχείου dyndep στη διαδρομή { $path }.
runner.io.dyndep.corrupt = Το παραγόμενο αρχείο dyndep στη διαδρομή { $path } δεν ταιριάζει με το αναμενόμενο περιεχόμενο· αφαιρέστε μόνο αυτό το αρχείο και δοκιμάστε ξανά.
runner.io.dyndep.temp_collisions = Δεν ήταν δυνατή η δημιουργία μοναδικού προσωρινού αρχείου dyndep για τη διαδρομή { $path } μετά από επανειλημμένες συγκρούσεις ονομάτων.
runner.io.dyndep.too_large = Το παραγόμενο αρχείο dyndep στη διαδρομή { $path } υπερβαίνει το όριο επαλήθευσης των { $limit } byte.

# Διαγνωστικά δηλωτικού.
manifest.parse = Η ανάλυση του δηλωτικού απέτυχε.
manifest.structure_error = Σφάλμα δομής του δηλωτικού στο { $name }: { $details }
manifest.yaml.parse = Σφάλμα ανάλυσης YAML στη γραμμή { $line }, στήλη { $column }: { $details }
manifest.yaml.label = μη έγκυρο YAML
manifest.yaml.hint.tabs = Το YAML δεν επιτρέπει στηλοθέτες· χρησιμοποιήστε κενά για την εσοχή.
manifest.yaml.hint.list_item = Τα στοιχεία λίστας YAML πρέπει να ξεκινούν με «-» και να έχουν σωστή εσοχή.
manifest.yaml.hint.expected_colon = Αυτό μοιάζει με καταχώριση αντιστοίχισης· λείπει «:» μετά το κλειδί.
manifest.yaml.hint.mapping_values = Οι αντιστοιχίσεις YAML απαιτούν τιμή μετά το «:» (ή ένθετο μπλοκ).
manifest.yaml.hint.invalid_token = Το λεκτικό YAML είναι μη έγκυρο ή απροσδόκητο.
manifest.yaml.hint.escape = Διαφύγετε τις ανάστροφες καθέτους ή αφαιρέστε τις μη έγκυρες ακολουθίες διαφυγής.
manifest.env.missing = Μια απαιτούμενη μεταβλητή περιβάλλοντος δεν έχει οριστεί.
manifest.env.invalid_utf8 = Μια μεταβλητή περιβάλλοντος περιέχει μη έγκυρο UTF-8.
manifest.vars.not_object = Το `vars` του δηλωτικού πρέπει να είναι αντιστοίχιση ή αντικείμενο.
manifest.vars.reserved_name = Το κλειδί `vars` '{ $name }' του μανιφέστου είναι δεσμευμένο για ενσωματωμένη βοηθητική συνάρτηση προτύπων· μετονομάστε τη μεταβλητή.
manifest.read_failed = Δεν ήταν δυνατή η ανάγνωση του δηλωτικού από { $path }.
manifest.resolve_workspace_root = Δεν ήταν δυνατός ο προσδιορισμός της ρίζας του χώρου εργασίας.
manifest.workspace_non_utf8 = Η ριζική διαδρομή του χώρου εργασίας «{ $path }» δεν είναι έγκυρο UTF-8.
manifest.path_non_utf8 = Η διαδρομή του δηλωτικού «{ $manifest }» δεν είναι έγκυρο UTF-8: { $path }.
manifest.path_missing_name = Η διαδρομή δηλωτικού «{ $path }» δεν έχει όνομα αρχείου.
manifest.open_workspace_failed = Δεν ήταν δυνατό το άνοιγμα του χώρου εργασίας { $workspace } για το δηλωτικό { $manifest }.
manifest.foreach.not_iterable = Η έκφραση `foreach` δεν είναι επαναλήψιμη.
manifest.foreach.serialise_item = Δεν ήταν δυνατή η σειριοποίηση του στοιχείου της `foreach`.
manifest.when.empty = Η έκφραση `when` δεν πρέπει να είναι κενή.
manifest.when.eval_error = Δεν ήταν δυνατή η αποτίμηση της έκφρασης `when` «{ $expr }».
manifest.when.template_error = Δεν ήταν δυνατή η απόδοση του προτύπου `when` «{ $expr }».
manifest.target.vars_not_object = Το `vars` του στόχου πρέπει να είναι αντικείμενο· ελήφθη { $value }.
manifest.vars.entry_not_object = Μια καταχώριση `vars` του δηλωτικού πρέπει να είναι αντικείμενο.
manifest.field_not_string = Το πεδίο «{ $field }» πρέπει να είναι συμβολοσειρά.
manifest.expression.parse_error = Δεν ήταν δυνατή η ανάλυση της έκφρασης { $name }.
manifest.expression.eval_error = Δεν ήταν δυνατή η αποτίμηση της έκφρασης { $name }.

# Διαγνωστικά μακροεντολών του δηλωτικού.
manifest.macro.signature_missing_identifier = Από την υπογραφή της μακροεντολής λείπει αναγνωριστικό.
manifest.macro.signature_missing_params = Από την υπογραφή της μακροεντολής λείπουν παράμετροι.
manifest.macro.compile_failed = Δεν ήταν δυνατή η μεταγλώττιση της μακροεντολής { $name }.
manifest.macro.sequence_invalid = Οι μακροεντολές πρέπει να ορίζονται ως αντιστοίχιση ονομάτων σε πρότυπα.
manifest.macro.register_failed = Δεν ήταν δυνατή η καταχώριση των μακροεντολών του δηλωτικού.
manifest.macro.not_initialised = Το περιβάλλον μακροεντολών δεν έχει αρχικοποιηθεί.
manifest.macro.caller_invalid = Ο καλών της μακροεντολής πρέπει να είναι συμβολοσειρά.
manifest.macro.template_load_failed = Δεν ήταν δυνατή η φόρτωση του προτύπου της μακροεντολής.
manifest.macro.init_failed = Δεν ήταν δυνατή η αρχικοποίηση του περιβάλλοντος μακροεντολών.
manifest.macro.missing = Η μακροεντολή { $name } λείπει.

# Σφάλματα μοτίβων glob στο δηλωτικό.
manifest.glob.unmatched_brace = Μη έγκυρο μοτίβο glob «{ $pattern }»: «{ $character }» χωρίς ταίρι στη θέση { $position }.
manifest.glob.invalid_pattern = Μη έγκυρο μοτίβο glob «{ $pattern }»: { $detail }.
manifest.glob.unknown_pattern_error = άγνωστο σφάλμα μοτίβου.
manifest.glob.io_failed = Το glob απέτυχε για «{ $pattern }»: { $detail }.
manifest.glob.unknown_io_error = άγνωστο σφάλμα εισόδου/εξόδου.
manifest.command_list_empty = Το πεδίο «command» δεν πρέπει να είναι κενό: δώστε μια συμβολοσειρά εντολής ή μια μη κενή λίστα.

# Σφάλματα της ενδιάμεσης αναπαράστασης.
ir.rule_not_found = Ο κανόνας «{ $rule }» στον οποίο παραπέμπει ο στόχος «{ $target }» δεν βρέθηκε.
ir.multiple_rules = Ο στόχος «{ $target }» πρέπει να παραπέμπει σε έναν μόνο κανόνα· ελήφθη { $rules }.
ir.empty_rule = Ο στόχος «{ $target }» πρέπει να παραπέμπει σε κανόνα.
ir.duplicate_outputs = Εντοπίστηκαν διπλότυπες έξοδοι: { $outputs }.
ir.circular_dependency = Εντοπίστηκε κυκλική εξάρτηση: { $cycle }.
ir.action_serialisation = Δεν ήταν δυνατή η σειριοποίηση της ενέργειας: { $details }.
ir.invalid_command = Μη έγκυρη παρεμβολή στην εντολή: { $snippet }.

# Σφάλματα παραγωγής αρχείων Ninja.
ninja_gen.missing_action = Λείπει η ενέργεια «{ $id }» στην οποία παραπέμπει ακμή δόμησης.
ninja_gen.format = Δεν ήταν δυνατή η μορφοποίηση της εξόδου του δηλωτικού Ninja.
ninja_gen.dyndep_files_required = Αυτή η δόμηση απαιτεί παραγόμενο πακέτο Ninja· χρησιμοποιήστε `netsuke build`, `netsuke clean` ή `netsuke generate`, ώστε να δημιουργηθούν τα αρχεία dyndep.
ninja_gen.reserved_output_path = Η διαδρομή '{ $path }' είναι δεσμευμένη για την κατάσταση σειριακών εξαρτήσεων του Netsuke.
ninja_gen.unsupported_path_character = Η διαδρομή '{ $path }' περιέχει τον μη υποστηριζόμενο χαρακτήρα διαδρομής Ninja '{ $character }'.

# Έλεγχος μοτίβων κόμβων.
host_pattern.empty = Το μοτίβο κόμβου δεν πρέπει να είναι κενό.
host_pattern.contains_scheme = Το μοτίβο κόμβου «{ $pattern }» δεν πρέπει να περιέχει σχήμα URL.
host_pattern.contains_slash = Το μοτίβο κόμβου «{ $pattern }» δεν πρέπει να περιέχει «/».
host_pattern.missing_suffix = Το μοτίβο κόμβου «{ $pattern }» πρέπει να περιέχει κατάληξη μετά το «*.».
host_pattern.empty_label = Το μοτίβο κόμβου «{ $pattern }» περιέχει κενή ετικέτα.
host_pattern.invalid_chars = Το μοτίβο κόμβου «{ $pattern }» περιέχει μη έγκυρους χαρακτήρες.
host_pattern.invalid_label_edge = Οι ετικέτες του μοτίβου κόμβου «{ $pattern }» δεν πρέπει να ξεκινούν ή να τελειώνουν με «-».
host_pattern.label_too_long = Το μοτίβο κόμβου «{ $pattern }» περιέχει ετικέτα μεγαλύτερη από 63 χαρακτήρες.
host_pattern.too_long = Το μοτίβο κόμβου «{ $pattern }» υπερβαίνει το όριο των 255 χαρακτήρων.

# Πολιτική δικτύου.
network_policy.scheme.empty = Το σχήμα δεν πρέπει να είναι κενό.
network_policy.scheme.invalid = Το σχήμα «{ $scheme }» περιέχει μη έγκυρους χαρακτήρες.
network_policy.allowlist.empty = Ο κατάλογος επιτρεπόμενων κόμβων δεν πρέπει να είναι κενός.
network_policy.scheme.not_allowed = Το σχήμα «{ $scheme }» δεν επιτρέπεται.
network_policy.missing_host = Από τη διεύθυνση URL λείπει ο κόμβος.
network_policy.host.blocked = Ο κόμβος «{ $host }» αποκλείεται από την πολιτική.
network_policy.host.not_allowlisted = Ο κόμβος «{ $host }» δεν περιλαμβάνεται στον κατάλογο επιτρεπόμενων.

# Ρυθμίσεις της τυπικής βιβλιοθήκης.
stdlib.config.default_fetch_cache_invalid = Η προεπιλεγμένη διαδρομή της κρυφής μνήμης fetch πρέπει να είναι σχετική.
stdlib.config.default_which_cache_invalid = Η προεπιλεγμένη χωρητικότητα της κρυφής μνήμης which πρέπει να είναι θετική.
stdlib.config.workspace_root_absolute = Η ριζική διαδρομή του χώρου εργασίας πρέπει να είναι απόλυτη.
stdlib.config.fetch_response_limit_positive = Το όριο απόκρισης του fetch πρέπει να είναι θετικό.
stdlib.config.command_output_limit_positive = Το όριο καταγραφής της εξόδου εντολών πρέπει να είναι θετικό.
stdlib.config.command_stream_limit_positive = Το όριο ροής εντολών πρέπει να είναι θετικό.
stdlib.config.which_cache_capacity_positive = Η χωρητικότητα της κρυφής μνήμης which πρέπει να είναι θετική.
stdlib.config.skip_dir_empty = Οι καταχωρίσεις καταλόγων προς παράλειψη δεν πρέπει να είναι κενές.
stdlib.config.skip_dir_navigation = Οι καταχωρίσεις καταλόγων προς παράλειψη δεν πρέπει να περιέχουν «..».
stdlib.config.skip_dir_separator = Οι καταχωρίσεις καταλόγων προς παράλειψη δεν πρέπει να περιέχουν διαχωριστικά διαδρομής.
stdlib.config.fetch_cache_empty = Η διαδρομή της κρυφής μνήμης fetch δεν πρέπει να είναι κενή.
stdlib.config.fetch_cache_not_relative = Η διαδρομή της κρυφής μνήμης fetch πρέπει να είναι σχετική· ελήφθη { $path }.
stdlib.config.fetch_cache_escapes = Η διαδρομή της κρυφής μνήμης fetch δεν πρέπει να βγαίνει έξω από τον χώρο εργασίας: { $path }.
stdlib.config.open_workspace_root = Δεν ήταν δυνατό το άνοιγμα του τρέχοντος καταλόγου ως ρίζας του χώρου εργασίας της stdlib.
stdlib.config.resolve_cwd = Δεν ήταν δυνατός ο προσδιορισμός του τρέχοντος καταλόγου ως ρίζας του χώρου εργασίας της stdlib.
stdlib.config.cwd_non_utf8 = Ο τρέχων κατάλογος περιέχει τμήματα που δεν είναι UTF-8: { $path }.

# Διαγνωστικά του βοηθήματος fetch.
stdlib.fetch.url_invalid = Μη έγκυρη διεύθυνση URL «{ $url }»: { $details }.
stdlib.fetch.disallowed = Η διεύθυνση URL «{ $url }» δεν επιτρέπεται: { $details }.
stdlib.fetch.failed = Δεν ήταν δυνατή η λήψη του «{ $url }»: { $details }.
stdlib.fetch.cache_read_failed = Δεν ήταν δυνατή η ανάγνωση της καταχώρισης κρυφής μνήμης «{ $name }»: { $details }.
stdlib.fetch.cache_open_failed = Δεν ήταν δυνατό το άνοιγμα της καταχώρισης κρυφής μνήμης «{ $name }»: { $details }.
stdlib.fetch.response_read_failed = Δεν ήταν δυνατή η ανάγνωση της απόκρισης από «{ $url }»: { $details }.
stdlib.fetch.response_buffer_overflow = Υπερχείλιση ενδιάμεσης μνήμης κατά την ανάγνωση του «{ $url }».
stdlib.fetch.cache_write_failed = Δεν ήταν δυνατή η εγγραφή της κρυφής μνήμης για «{ $url }»: { $details }.
stdlib.fetch.response_limit_exceeded = Η απόκριση από «{ $url }» υπερέβη το όριο των { $limit } byte.
stdlib.fetch.cache_limit_exceeded = Η αποθηκευμένη απόκριση «{ $name }» υπερέβη το όριο των { $limit } byte.
stdlib.fetch.io_failed = Η ενέργεια «{ $action }» απέτυχε για { $path }: { $details }.
stdlib.fetch.action.sync_cache = συγχρονισμός της κρυφής μνήμης fetch
stdlib.fetch.action.create_cache_dir = δημιουργία του καταλόγου κρυφής μνήμης fetch
stdlib.fetch.action.open_cache_dir = άνοιγμα του καταλόγου κρυφής μνήμης fetch
stdlib.fetch.action.stat_cache = ανάκτηση στοιχείων της καταχώρισης κρυφής μνήμης fetch
stdlib.fetch.action.open_cache_entry = άνοιγμα της καταχώρισης κρυφής μνήμης fetch

# Διαγνωστικά του βοηθήματος εντολών.
stdlib.command.location = εντολή «{ $command }» στο πρότυπο «{ $template }»
stdlib.command.spawn_failed = Η { $location } δεν μπόρεσε να εκκινήσει: { $details }.
stdlib.command.io_failed = Η { $location } απέτυχε: { $details }.
stdlib.command.closed_input_early = Η είσοδος έκλεισε πριν ολοκληρωθεί η εγγραφή προς την εντολή.
stdlib.command.broken_pipe = Διακοπή διοχέτευσης ενώ εκτελούνταν η { $location }: { $details }.
stdlib.command.terminated_by_signal = Η { $location } τερματίστηκε από σήμα.
stdlib.command.exited_with_status = Η { $location } τερματίστηκε με κατάσταση { $status }.
stdlib.command.output_limit_exceeded = Η { $location } υπερέβη το όριο { $mode } των { $limit } byte για { $stream }.
stdlib.command.timeout = Η { $location } υπερέβη το χρονικό όριο των { $seconds } δευτερολέπτων.
stdlib.command.exit_status_suffix = (κατάσταση εξόδου { $status })
stdlib.command.signal_suffix = (τερματίστηκε από σήμα)
stdlib.command.shell.empty = Η εντολή κελύφους δεν πρέπει να είναι κενή.
stdlib.command.grep.empty_pattern = Το μοτίβο του grep δεν πρέπει να είναι κενό.
stdlib.command.grep.flags_not_string = Οι σημαίες του grep πρέπει να είναι συμβολοσειρές.
stdlib.command.quote.invalid = Δεν ήταν δυνατή η χρήση εισαγωγικών για το { $arg }: { $details }.
stdlib.command.quote.line_break = Ορίσματα με χαρακτήρες επαναφοράς ή αλλαγής γραμμής δεν μπορούν να τεθούν με ασφάλεια σε εισαγωγικά.
stdlib.command.input_undefined = Η τιμή εισόδου δεν είναι ορισμένη.
stdlib.command.tempfile.root_required = Για τη δημιουργία προσωρινών αρχείων εντολών απαιτείται η ρίζα του χώρου εργασίας.
stdlib.command.tempfile.create_failed = Δεν ήταν δυνατή η δημιουργία του προσωρινού αρχείου εντολής: { $details }.
stdlib.command.options.invalid_utf8 = Το κλειδί επιλογής της εντολής πρέπει να είναι έγκυρο UTF-8.
stdlib.command.option.mode_not_string = Η κατάσταση εξόδου πρέπει να είναι συμβολοσειρά.
stdlib.command.options.invalid_type = Οι επιλογές της εντολής πρέπει να είναι αντικείμενο.
stdlib.command.output.mode_unsupported = Μη υποστηριζόμενη κατάσταση εξόδου «{ $mode }».
stdlib.command.output.mode.capture = καταγραφή
stdlib.command.output.mode.streaming = συνεχής ροή
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Διαγνωστικά του βοηθήματος διαδρομών.
stdlib.path.io.failed = Η ενέργεια «{ $action }» απέτυχε για { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Η ενέργεια «{ $action }» απέτυχε για { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Η ενέργεια «{ $action }» απέτυχε για { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = δεν βρέθηκε
stdlib.path.io.permission_denied = δεν επιτρέπεται η πρόσβαση
stdlib.path.io.already_exists = υπάρχει ήδη
stdlib.path.io.invalid_input = μη έγκυρη είσοδος
stdlib.path.io.invalid_data = μη έγκυρα δεδομένα
stdlib.path.io.timed_out = έληξε το χρονικό όριο
stdlib.path.io.interrupted = διακόπηκε
stdlib.path.io.would_block = θα προκαλούσε φραγή
stdlib.path.io.write_zero = γράφτηκαν μηδέν byte
stdlib.path.io.unexpected_eof = απροσδόκητο τέλος αρχείου
stdlib.path.io.broken_pipe = διακοπή διοχέτευσης
stdlib.path.io.connection_refused = άρνηση σύνδεσης
stdlib.path.io.connection_reset = επαναφορά σύνδεσης
stdlib.path.io.connection_aborted = ματαίωση σύνδεσης
stdlib.path.io.not_connected = χωρίς σύνδεση
stdlib.path.io.addr_in_use = η διεύθυνση χρησιμοποιείται ήδη
stdlib.path.io.addr_not_available = η διεύθυνση δεν είναι διαθέσιμη
stdlib.path.io.out_of_memory = εξαντλήθηκε η μνήμη
stdlib.path.io.unsupported = δεν υποστηρίζεται
stdlib.path.io.file_too_large = το αρχείο είναι πολύ μεγάλο
stdlib.path.io.resource_busy = ο πόρος είναι απασχολημένος
stdlib.path.io.executable_busy = το εκτελέσιμο είναι απασχολημένο
stdlib.path.io.deadlock = αδιέξοδο
stdlib.path.io.crosses_devices = διασχίζει συσκευές
stdlib.path.io.too_many_links = υπερβολικά πολλοί σύνδεσμοι
stdlib.path.io.invalid_filename = μη έγκυρο όνομα αρχείου
stdlib.path.io.arg_list_too_long = υπερβολικά μεγάλος κατάλογος ορισμάτων
stdlib.path.io.stale_handle = παρωχημένος χειριστής δικτυακού αρχείου
stdlib.path.io.storage_full = ο χώρος αποθήκευσης είναι πλήρης
stdlib.path.io.not_seekable = δεν επιτρέπει αναζήτηση θέσης
stdlib.path.io.network_down = το δίκτυο δεν λειτουργεί
stdlib.path.io.network_unreachable = το δίκτυο δεν είναι προσβάσιμο
stdlib.path.io.host_unreachable = ο κόμβος δεν είναι προσβάσιμος
stdlib.path.io.other = σφάλμα εισόδου/εξόδου
stdlib.path.action.canonicalize = κανονικοποίηση
stdlib.path.action.open_directory = άνοιγμα καταλόγου
stdlib.path.action.stat = ανάκτηση στοιχείων
stdlib.path.action.read = ανάγνωση
stdlib.path.action.open_file = άνοιγμα αρχείου
stdlib.path.with_suffix.empty_separator = Το with_suffix απαιτεί μη κενό διαχωριστικό.
stdlib.path.relative_to.mismatch = Το { $path } δεν είναι σχετικό ως προς το { $root }.
stdlib.path.expanduser.unsupported = Η ανάπτυξη του ~ για συγκεκριμένο χρήστη δεν υποστηρίζεται.
stdlib.path.expanduser.no_home = Δεν είναι δυνατή η ανάπτυξη του ~: δεν έχει οριστεί καμία μεταβλητή περιβάλλοντος για τον προσωπικό κατάλογο.
stdlib.path.contents.unsupported_encoding = Μη υποστηριζόμενη κωδικοποίηση «{ $encoding }».
stdlib.path.hash.unsupported_algorithm = Μη υποστηριζόμενος αλγόριθμος κατακερματισμού «{ $algorithm }».
stdlib.path.hash.unsupported_algorithm_legacy = Μη υποστηριζόμενος αλγόριθμος κατακερματισμού «{ $algorithm }» (ενεργοποιήστε τη δυνατότητα «{ $feature }»).

# Διαγνωστικά των βοηθημάτων συλλογών.
stdlib.collections.flatten.expected_sequence = Το flatten περίμενε στοιχεία ακολουθίας αλλά βρήκε { $kind }.
stdlib.collections.group_by.empty_attribute = Το group_by απαιτεί μη κενό γνώρισμα.
stdlib.collections.group_by.unresolved = Το group_by δεν μπόρεσε να εντοπίσει το «{ $attr }» σε στοιχείο τύπου { $kind }.

# Διαγνωστικά των βοηθημάτων χρόνου.
stdlib.time.offset.invalid = Η μετατόπιση now «{ $offset }» δεν είναι έγκυρη: αναμενόταν «+HH:MM[:SS]» ή «Z».
stdlib.time.timedelta.overflow = Υπερχείλιση timedelta κατά την πρόσθεση του { $component }.
stdlib.time.label.weeks = εβδομάδες
stdlib.time.label.days = ημέρες
stdlib.time.label.hours = ώρες
stdlib.time.label.minutes = λεπτά
stdlib.time.label.seconds = δευτερόλεπτα
stdlib.time.label.milliseconds = χιλιοστά του δευτερολέπτου
stdlib.time.label.microseconds = εκατομμυριοστά του δευτερολέπτου
stdlib.time.label.nanoseconds = δισεκατομμυριοστά του δευτερολέπτου

# Διαγνωστικά του βοηθήματος which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] η εντολή «{ $command }» δεν βρέθηκε μετά τον έλεγχο { $count } καταχωρίσεων του PATH. Προεπισκόπηση: { $preview }
stdlib.which.not_found.hint.cwd_auto = Τα κενά τμήματα του PATH αγνοούνται· χρησιμοποιήστε cwd_mode="auto" για να συμπεριληφθεί ο κατάλογος εργασίας.
stdlib.which.not_found.hint.cwd_always = Ορίστε cwd_mode="always" για να συμπεριληφθεί ο τρέχων κατάλογος.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] η εντολή «{ $command }» στο «{ $path }» λείπει ή δεν είναι εκτελέσιμη.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <κενό>
stdlib.which.path_entry.non_utf8 = Η καταχώριση αρ. { $index } του PATH περιέχει χαρακτήρες που δεν είναι UTF-8· το Netsuke απαιτεί διαδρομές UTF-8.
stdlib.which.command.empty = Το which απαιτεί μη κενή συμβολοσειρά.
stdlib.which.cwd_mode.invalid = Το cwd_mode πρέπει να είναι «auto», «always» ή «never»· ελήφθη «{ $mode }».
stdlib.which.cwd.resolve_failed = Δεν ήταν δυνατός ο προσδιορισμός του τρέχοντος καταλόγου: { $details }.
stdlib.which.cwd.non_utf8 = Ο τρέχων κατάλογος περιέχει τμήματα που δεν είναι UTF-8.
stdlib.which.canonicalize_failed = Δεν ήταν δυνατή η κανονικοποίηση του «{ $path }»: { $details }.
stdlib.which.is_executable = Δεν ήταν δυνατός ο έλεγχος του αν το «{ $path }» είναι εκτελέσιμο: { $details }.
stdlib.which.canonicalize_non_utf8 = Η κανονική διαδρομή περιέχει τμήματα που δεν είναι UTF-8.
stdlib.which.workspace_non_utf8 = Η διαδρομή του χώρου εργασίας περιέχει τμήματα που δεν είναι UTF-8 κατά την επίλυση της εντολής «{ $command }»: { $path }.
stdlib.which.walkdir_error = Σφάλμα διάσχισης του χώρου εργασίας κατά την επίλυση της εντολής: { $details }.

# Καταχώριση της τυπικής βιβλιοθήκης.
stdlib.register.open_dir = Δεν ήταν δυνατό το άνοιγμα του τρέχοντος καταλόγου για την καταχώριση της stdlib.
stdlib.register.resolve_dir = Δεν ήταν δυνατός ο προσδιορισμός του τρέχοντος καταλόγου για την καταχώριση της stdlib.
stdlib.register.dir_non_utf8 = Ο τρέχων κατάλογος περιέχει τμήματα που δεν είναι UTF-8: { $path }.

# Αναφορά κατάστασης για την προσβάσιμη έξοδο.
status.state.pending = σε αναμονή
status.state.running = σε εξέλιξη
status.state.done = ολοκληρώθηκε
status.state.failed = απέτυχε
status.stage.label = Στάδιο { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Εργασία { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Ανάγνωση του αρχείου δηλωτικού
status.stage.initial_yaml_parsing = Ανάλυση του εγγράφου YAML
status.stage.template_expansion = Ανάπτυξη των οδηγιών προτύπου
status.stage.final_rendering = Αποσειριοποίηση και απόδοση των τιμών του δηλωτικού
status.stage.ir_generation_validation = Κατασκευή και έλεγχος του γραφήματος εξαρτήσεων
status.stage.ninja_synthesis = Σύνθεση του σχεδίου δόμησης Ninja
status.stage.ninja_synthesis_execute = Σύνθεση του σχεδίου Ninja και εκτέλεση του { $tool }
status.stage.graph_rendering = Απόδοση του τεχνουργήματος γραφήματος
status.stage.graph_rendering_with_tool = Απόδοση του { $tool }
status.complete = { $tool }: ολοκληρώθηκε.
status.timing.summary_header = Σύνοψη χρόνων ανά στάδιο:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Συνολικός χρόνος της ροής: { $duration }
status.tool.build = Δόμηση
status.tool.clean = Καθαρισμός
status.tool.graph = Γράφημα
status.tool.graph_html = Γράφημα (HTML)
status.tool.generate = Δημιουργία
status.tool.help_targets = Βοήθεια στόχων

# Κείμενα της απόδοσης του γραφήματος σε HTML.
graph.html.title = Γράφημα δόμησης του Netsuke
graph.html.heading = Γράφημα δόμησης του Netsuke
graph.html.description = Γράφημα δόμησης που αποδόθηκε από το Netsuke
graph.html.outline.summary = Στόχοι και εξαρτήσεις (διάρθρωση σε κείμενο)
graph.html.outline.no_inputs = Καμία είσοδος
graph.html.noscript.notice = Η JavaScript είναι απενεργοποιημένη. Η παραπάνω διάρθρωση σε κείμενο περιέχει ολόκληρο το γράφημα· ακολουθεί ο πηγαίος κώδικας DOT.

# Σημασιολογικά προθέματα για την προσβάσιμη έξοδο.
semantic.prefix.error = Σφάλμα:
semantic.prefix.warning = Προειδοποίηση:
semantic.prefix.success = Επιτυχία:
semantic.prefix.info = Πληροφορία:
semantic.prefix.timing = Χρόνος:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Παραδείγματα πληθυντικών μορφών για μεταφραστές.
# Τα ελληνικά χρησιμοποιούν τις κατηγορίες CLDR `one` και `other`, όπως και η
# γλώσσα προέλευσης.
example.files_processed = { $count ->
    [one] Επεξεργάστηκε { $count } αρχείο.
   *[other] Επεξεργάστηκαν { $count } αρχεία.
}

example.errors_found = { $count ->
    [0] Δεν βρέθηκαν σφάλματα.
    [one] Βρέθηκε { $count } σφάλμα.
   *[other] Βρέθηκαν { $count } σφάλματα.
}
