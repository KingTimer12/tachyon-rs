import { Tachyon } from 'tachyon-rs'

new Tachyon()
  .get('/', 'Hello Tachyon!')
  .get('/mingau', {
    response: 'mingau'
  })
  .listen(3000)
